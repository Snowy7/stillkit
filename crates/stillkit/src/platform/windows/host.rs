//! The few desktop queries an app host needs (`el::run`): screen size, modifier state and an
//! idle wait on this thread's messages, which also ends at the one deadline [`wake_at`] set.

use std::cell::Cell;
use std::time::Instant;

use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetSystemMetrics, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx,
    PM_REMOVE, PeekMessageW, QS_ALLINPUT, SM_CXSCREEN, SM_CYSCREEN, TranslateMessage,
};

use crate::el::Modifiers;

thread_local! {
    /// When [`wait`] returns even if no message comes.
    static WAKE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// The primary screen's size in device pixels.
pub fn screen_size() -> (i32, i32) {
    // SAFETY: plain metric queries.
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

fn down(vk: u16) -> bool {
    // SAFETY: a query of this thread's key state.
    unsafe { GetKeyState(i32::from(vk)) < 0 }
}

/// Whether Shift is held, as this thread's input last saw it.
pub fn shift_down() -> bool {
    down(VK_SHIFT.0)
}

/// The modifier keys held, as this thread's input last saw them (the key being handled).
pub fn modifiers() -> Modifiers {
    Modifiers {
        shift: down(VK_SHIFT.0),
        ctrl: down(VK_CONTROL.0),
        alt: down(VK_MENU.0),
        meta: down(VK_LWIN.0) || down(VK_RWIN.0),
    }
}

/// Makes [`wait`] return by `deadline` though no message comes (`None`: only messages wake
/// it). The wait's own timeout does it: no timer, thread or handle.
pub fn wake_at(deadline: Option<Instant>) {
    WAKE.set(deadline);
}

/// Sleeps until a window message arrives (or the [`wake_at`] deadline passes), then
/// dispatches every message waiting.
pub fn wait() {
    let millis = WAKE.get().map_or(INFINITE, |at| {
        let left = at.saturating_duration_since(Instant::now());
        // Rounded up: waking a millisecond early would only wait again.
        left.as_micros().div_ceil(1000).min(u128::from(INFINITE - 1)) as u32
    });
    pump(millis);
}

/// [`wait`], but for at most `timeout`.
pub fn wait_for(timeout: std::time::Duration) {
    pump(timeout.as_millis().min(u128::from(INFINITE - 1)) as u32);
}

fn pump(millis: u32) {
    // SAFETY: a standard message pump on this thread, with no handles to outlive.
    unsafe {
        MsgWaitForMultipleObjectsEx(None, millis, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    crate::waker::rearm();
}

/// Whether the user asked for less motion (Settings > Accessibility > Visual effects >
/// Animation effects off, `SPI_GETCLIENTAREAANIMATION`).
pub fn reduced_motion() -> bool {
    !super::screen::animations_enabled()
}

/// The [`Waker`](crate::Waker)'s native half: a thread message posted to the UI thread, which
/// ends `MsgWaitForMultipleObjectsEx`. Dispatching it does nothing (it has no window), and a
/// modal loop that drops it returns through the pump, which re-arms the waker anyway.
pub(crate) struct Poster(u32);

impl Poster {
    pub(crate) fn new() -> Self {
        // SAFETY: plain query of the calling thread's ID.
        Self(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() })
    }

    pub(crate) fn post(&self) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};
        // SAFETY: posts a message with no pointers to a thread ID; failure (the thread
        // ended) only means nobody waits any more.
        let _ = unsafe { PostThreadMessageW(self.0, WM_APP + 0x5741, WPARAM(0), LPARAM(0)) };
    }
}
