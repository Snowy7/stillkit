//! The thread's event queue and the AppKit pump. Views and window delegates turn AppKit's
//! callbacks into queued [`Event`]s (never holding a borrow across a call into AppKit, which
//! may call back); [`wait`] sleeps in `nextEventMatchingMask` until a distant date, the
//! [`set_wake`] deadline, or the next display frame while an animation runs.
use super::surface::{Event, SurfaceId};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};

/// A surface's display-rate frames: queued while `active` and before `end`.
struct Producer {
    id: SurfaceId,
    active: Rc<Cell<bool>>,
    end: Instant,
}

thread_local! {
    static EVENTS: RefCell<VecDeque<(SurfaceId, Event)>> = const { RefCell::new(VecDeque::new()) };
    static WAKE: Cell<Option<Instant>> = const { Cell::new(None) };
    static FRAMES: RefCell<Vec<Producer>> = const { RefCell::new(Vec::new()) };
    static LAST_FRAME: Cell<Option<Instant>> = const { Cell::new(None) };
    static LAUNCHED: Cell<Option<NSApplicationActivationPolicy>> = const { Cell::new(None) };
}

pub(super) fn push(id: SurfaceId, event: Event) {
    EVENTS.with_borrow_mut(|q| q.push_back((id, event)));
}

pub(super) fn discard(id: SurfaceId) {
    EVENTS.with_borrow_mut(|q| q.retain(|(i, _)| *i != id));
}

pub(super) fn set_wake(deadline: Option<Instant>) {
    WAKE.set(deadline);
}

/// The shared application, launched once: a regular app (Dock icon, menu bar) once an app
/// window exists, an accessory (no Dock icon) for popups, bars and desktop surfaces alone.
pub(super) fn app(mtm: MainThreadMarker, regular: bool) -> objc2::rc::Retained<NSApplication> {
    let app = NSApplication::sharedApplication(mtm);
    let want = if regular { NSApplicationActivationPolicy::Regular } else { NSApplicationActivationPolicy::Accessory };
    match LAUNCHED.get() {
        None => {
            app.setActivationPolicy(want);
            app.finishLaunching();
            LAUNCHED.set(Some(want));
        }
        Some(NSApplicationActivationPolicy::Accessory) if regular => {
            app.setActivationPolicy(want);
            LAUNCHED.set(Some(want));
        }
        _ => {}
    }
    app
}

/// Keeps queuing `Frame` for `id` until `active` turns false or `end` passes.
pub(super) fn frames(id: SurfaceId, active: Rc<Cell<bool>>, end: Instant) {
    FRAMES.with_borrow_mut(|f| f.push(Producer { id, active, end }));
}

/// When the next frame is due, if anything animates.
fn next_frame() -> Option<Instant> {
    let now = Instant::now();
    let any = FRAMES.with_borrow_mut(|f| {
        f.retain(|p| p.active.get() && now < p.end);
        !f.is_empty()
    });
    #[cfg(feature = "gpu")]
    let any = any || super::gpu::animating();
    any.then(|| LAST_FRAME.get().map_or(now, |last| last + super::timing::display_frame()))
}

/// Queues the frames that are due (and presents animating in-process glass).
fn tick() {
    let now = Instant::now();
    if next_frame().is_none_or(|due| now < due) {
        return;
    }
    LAST_FRAME.set(Some(now));
    let due: Vec<SurfaceId> = FRAMES.with_borrow(|f| f.iter().map(|p| p.id).collect());
    for id in due {
        push(id, Event::Frame);
    }
    #[cfg(feature = "gpu")]
    super::gpu::tick();
}

/// Dispatches AppKit events: the first waited for until `until` (`None`: not at all), then
/// every one already waiting.
fn dispatch(mtm: MainThreadMarker, until: Option<Option<Instant>>) {
    if LAUNCHED.get().is_none() {
        return;
    }
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: an extern static Foundation defines.
    let mode = unsafe { NSDefaultRunLoopMode };
    let mut date = match until {
        None => NSDate::distantPast(),
        Some(None) => NSDate::distantFuture(),
        Some(Some(at)) => NSDate::dateWithTimeIntervalSinceNow(at.saturating_duration_since(Instant::now()).as_secs_f64()),
    };
    while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&date), mode, true) {
        // A Waker's event only ends the wait.
        let wake = event.r#type() == objc2_app_kit::NSEventType::ApplicationDefined
            && event.subtype().0 == super::host::WAKE_SUBTYPE;
        if !wake {
            app.sendEvent(&event);
        }
        date = NSDate::distantPast();
    }
    app.updateWindows();
}

/// Takes one queued event without blocking, dispatching AppKit's first when none is queued.
pub fn next_event() -> Option<(SurfaceId, Event)> {
    if EVENTS.with_borrow(VecDeque::is_empty)
        && let Some(mtm) = MainThreadMarker::new()
    {
        dispatch(mtm, None);
        tick();
    }
    EVENTS.with_borrow_mut(VecDeque::pop_front)
}

/// Sleeps until an event, the wake deadline, the next frame, or `limit`.
pub(super) fn wait(limit: Option<Duration>) {
    if !EVENTS.with_borrow(VecDeque::is_empty) {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        // No AppKit off the main thread: only the deadline can end the wait.
        let until = [WAKE.get(), limit.map(|l| Instant::now() + l)].into_iter().flatten().min();
        if let Some(until) = until {
            std::thread::sleep(until.saturating_duration_since(Instant::now()));
        }
        return;
    };
    let until = [WAKE.get(), next_frame(), limit.map(|l| Instant::now() + l)].into_iter().flatten().min();
    dispatch(mtm, Some(until));
    tick();
    crate::waker::rearm();
}
