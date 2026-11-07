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
