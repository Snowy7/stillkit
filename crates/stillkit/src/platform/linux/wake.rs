//! The one deadline `host::wake_at` sets: a timerfd the display wait polls beside the Wayland
//! fd. It is made on the first deadline and disarmed whenever none is pending, so an idle app
//! has nothing to wake it.

use std::cell::Cell;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::time::Instant;

thread_local! {
    static TIMER: Cell<Option<RawFd>> = const { Cell::new(None) };
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Keeps the descriptor open for the thread's life.
    static OWNED: std::cell::RefCell<Option<OwnedFd>> = const { std::cell::RefCell::new(None) };
}

fn timer() -> Option<RawFd> {
    if let Some(fd) = TIMER.get() {
        return Some(fd);
    }
    // SAFETY: plain syscall; the result is checked and owned below.
    let fd = unsafe { libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_NONBLOCK | libc::TFD_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // SAFETY: a fresh descriptor this thread now owns.
    OWNED.with_borrow_mut(|o| *o = Some(unsafe { OwnedFd::from_raw_fd(fd) }));
    TIMER.set(Some(fd));
    Some(fd)
}

/// Arms the timer for `deadline`, or disarms it (`None`).
pub(super) fn set(deadline: Option<Instant>) {
    if deadline.is_none() && !ARMED.get() {
        return;
    }
    let Some(fd) = timer() else { return };
    let left = deadline.map(|at| at.saturating_duration_since(Instant::now()));
    // A deadline already passed still has to fire: one nanosecond (zero disarms).
    let left = left.map(|d| d.max(std::time::Duration::from_nanos(1)));
    let spec = libc::itimerspec {
        it_interval: libc::timespec { tv_sec: 0, tv_nsec: 0 },
        it_value: libc::timespec {
            tv_sec: left.map_or(0, |d| d.as_secs() as libc::time_t),
            tv_nsec: left.map_or(0, |d| d.subsec_nanos() as libc::c_long),
        },
    };
    // SAFETY: a live timerfd and a valid itimerspec; the old value isn't asked for.
    unsafe { libc::timerfd_settime(fd, 0, &spec, std::ptr::null_mut()) };
    ARMED.set(deadline.is_some());
}

/// The timer's descriptor while a deadline is pending.
pub(super) fn armed() -> Option<RawFd> {
    ARMED.get().then(|| TIMER.get()).flatten()
}

/// The deadline passed: consume the expiry so the descriptor stops being readable.
pub(super) fn fired() {
    if let Some(fd) = TIMER.get() {
        let mut n = [0u8; 8];
        // SAFETY: reads at most 8 bytes into a local buffer from a non-blocking timerfd.
        unsafe { libc::read(fd, n.as_mut_ptr().cast(), n.len()) };
    }
    ARMED.set(false);
}

thread_local! {
    /// The [`Waker`](crate::Waker)'s eventfd, polled beside the display while it exists.
    static WAKER: Cell<Option<RawFd>> = const { Cell::new(None) };
}

/// The [`Waker`](crate::Waker)'s native half: an eventfd the display wait polls. Writes from
/// any thread make it readable; the wait reads it back to zero.
pub(crate) struct Poster(Option<std::sync::Arc<OwnedFd>>);

impl Poster {
    pub(crate) fn new() -> Self {
        // SAFETY: plain syscall; the result is checked and owned below.
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if fd < 0 {
            return Self(None);
        }
        WAKER.set(Some(fd));
        // SAFETY: a fresh descriptor; the Arc keeps it open while any Waker clone lives, and
        // a waker lives for its thread (`waker` caches it), so the polled fd stays valid.
        Self(Some(std::sync::Arc::new(unsafe { OwnedFd::from_raw_fd(fd) })))
    }

    pub(crate) fn post(&self) {
        use std::os::fd::AsRawFd;
        let Some(fd) = &self.0 else { return };
        let one = 1u64.to_ne_bytes();
        // SAFETY: writes 8 bytes from a local buffer to a live non-blocking eventfd.
        unsafe { libc::write(fd.as_raw_fd(), one.as_ptr().cast(), one.len()) };
    }
}

/// The waker's descriptor, once this thread made one.
pub(super) fn waker_fd() -> Option<RawFd> {
    WAKER.get()
}

/// The waker fired: read the counter back to zero.
pub(super) fn woken() {
    if let Some(fd) = WAKER.get() {
        let mut n = [0u8; 8];
        // SAFETY: reads at most 8 bytes into a local buffer from a non-blocking eventfd.
        unsafe { libc::read(fd, n.as_mut_ptr().cast(), n.len()) };
    }
}
