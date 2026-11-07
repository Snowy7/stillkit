use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use crate::menu::Edges;
/// A monitor as a popup sees it: where it may go, and how large its pixels are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub work: Edges,
    pub dpi: u32,
}

/// The monitor at (or nearest to) a screen point.
pub fn screen_at(x: i32, y: i32) -> Screen {
    // SAFETY: plain queries with valid out pointers.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let work = if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let w = info.rcWork;
            (w.left, w.top, w.right, w.bottom)
        } else {
            (i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2)
        };
        let (mut dpi, mut unused) = (96, 96);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused);
        Screen { work, dpi }
    }
}

pub(crate) fn animations_enabled() -> bool {
    let mut animations = windows::core::BOOL(1);
    // SAFETY: the system query writes one BOOL into our local.
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
        windows::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION, 0,
        Some((&raw mut animations).cast()), Default::default()) };
    animations.as_bool()
}
