#![cfg(windows)]
//! App windows report activation like every other surface. A host keeping a glass plate
//! directly under its window restacks it on `Event::Activated`, because activation raises the
//! window above whatever covered both; without the event the plate stayed under the other app
//! until the window moved. The message goes straight to a hidden window: nothing is activated.

use stillkit::{Event, Kind, Role, Surface, next_event};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WA_ACTIVE, WA_INACTIVE, WM_ACTIVATE};

fn events_for(surface: &Surface) -> Vec<Event> {
    std::iter::from_fn(next_event).filter(|(id, _)| *id == surface.id()).map(|(_, e)| e).collect()
}

#[test]
fn app_windows_report_activation_and_its_loss() {
    let app = Surface::new(Kind::App { min: (200, 120) }, "activation", Role::Window).unwrap();
    let _ = events_for(&app);
    // SAFETY: synchronous messages to this thread's own hidden window.
    unsafe { SendMessageW(app.hwnd(), WM_ACTIVATE, Some(WPARAM(WA_ACTIVE as usize)), Some(LPARAM(0))) };
    assert!(events_for(&app).iter().any(|e| matches!(e, Event::Activated)), "activation reaches the app");
    // SAFETY: as above.
    unsafe { SendMessageW(app.hwnd(), WM_ACTIVATE, Some(WPARAM(WA_INACTIVE as usize)), Some(LPARAM(0))) };
    let events = events_for(&app);
    assert!(events.iter().any(|e| matches!(e, Event::Deactivated)) && !events.iter().any(|e| matches!(e, Event::Activated)));
}
