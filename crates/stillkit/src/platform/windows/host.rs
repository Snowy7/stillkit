//! The few desktop queries an app host needs (`el::run`): screen size, modifier state and an
//! idle wait on this thread's messages.

use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetSystemMetrics, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx,
    PM_REMOVE, PeekMessageW, QS_ALLINPUT, SM_CXSCREEN, SM_CYSCREEN, TranslateMessage,
};

/// The primary screen's size in device pixels.
pub fn screen_size() -> (i32, i32) {
    // SAFETY: plain metric queries.
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

/// Whether Shift is held, as this thread's input last saw it.
pub fn shift_down() -> bool {
    // SAFETY: a query of this thread's key state.
    unsafe { GetKeyState(i32::from(VK_SHIFT.0)) < 0 }
}

/// Sleeps until a window message arrives, then dispatches every message waiting.
pub fn wait() {
    // SAFETY: a standard message pump on this thread, with no handles to outlive.
    unsafe {
        MsgWaitForMultipleObjectsEx(None, INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Whether the user asked for less motion (Settings > Accessibility > Visual effects >
/// Animation effects off, `SPI_GETCLIENTAREAANIMATION`).
pub fn reduced_motion() -> bool {
    !super::screen::animations_enabled()
}
