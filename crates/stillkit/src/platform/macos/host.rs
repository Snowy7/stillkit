//! The desktop queries an app host needs (`el::run`) and its idle wait: AppKit's
//! `nextEventMatchingMask` until a distant date (no wakes at all while idle), or until the one
//! deadline [`wake_at`] set, the run loop's own timeout.

use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSWorkspace};

use crate::el::Modifiers;

/// The primary screen's size in device pixels.
pub fn screen_size() -> (i32, i32) {
    let s = super::screen::primary();
    (s.size.0, s.size.1)
}

/// Whether Shift is held now.
pub fn shift_down() -> bool {
    modifiers().shift
}

/// The modifier keys held now. Command is `ctrl` (the shortcut key: Command-C copies, as
/// Ctrl+C does elsewhere), Option is `alt` and Control is `meta`.
pub fn modifiers() -> Modifiers {
    held(NSEvent::modifierFlags_class())
}

/// [`modifiers`] from an event's flags.
pub(super) fn held(f: NSEventModifierFlags) -> Modifiers {
    Modifiers {
        shift: f.contains(NSEventModifierFlags::Shift),
        ctrl: f.contains(NSEventModifierFlags::Command),
        alt: f.contains(NSEventModifierFlags::Option),
        meta: f.contains(NSEventModifierFlags::Control),
    }
}

/// Makes [`wait`] return by `deadline` though no event comes (`None`: only events wake it).
pub fn wake_at(deadline: Option<std::time::Instant>) {
    super::pump::set_wake(deadline);
}

/// Sleeps until an event arrives (or the [`wake_at`] deadline passes, or an animation's next
/// frame is due), then dispatches every event waiting.
pub fn wait() {
    super::pump::wait(None);
}

/// [`wait`], but for at most `timeout`.
pub fn wait_for(timeout: std::time::Duration) {
    super::pump::wait(Some(timeout));
}

/// Whether the user asked for less motion (System Settings > Accessibility > Display > Reduce
/// motion), read live.
pub fn reduced_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// The [`Waker`](crate::Waker)'s native half: an application-defined `NSEvent` posted to
/// AppKit's queue, which ends `nextEventMatchingMask`'s wait. Made on the main thread.
pub(crate) struct Poster(Option<std::ptr::NonNull<objc2_app_kit::NSApplication>>);

// SAFETY: the pointer is the process's shared NSApplication, which lives forever, and the only
// call made through it off the main thread is `postEvent:atStart:`, which AppKit documents as
// safe from subthreads.
unsafe impl Send for Poster {}
// SAFETY: as above; posting needs no exclusive access.
unsafe impl Sync for Poster {}

impl Poster {
    pub(crate) fn new() -> Self {
        let app = objc2::MainThreadMarker::new().map(objc2_app_kit::NSApplication::sharedApplication);
        // Retained::into_raw keeps the shared app retained for the process's life.
        Self(app.and_then(|app| std::ptr::NonNull::new(objc2::rc::Retained::into_raw(app))))
    }

    pub(crate) fn post(&self) {
        use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
        let Some(app) = self.0 else { return };
        let Some(event) = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::ApplicationDefined,
            objc2_foundation::NSPoint::ZERO,
            NSEventModifierFlags::empty(),
            0.0,
            0,
            None,
            WAKE_SUBTYPE,
            0,
            0,
        ) else {
            return;
        };
        // SAFETY: see `Poster`'s Send/Sync: the shared app is alive, and posting is
        // documented thread-safe.
        unsafe { app.as_ref() }.postEvent_atStart(&event, false);
    }
}

/// The subtype of the waker's application-defined event; the pump drops these.
pub(super) const WAKE_SUBTYPE: i16 = 0x5741;
