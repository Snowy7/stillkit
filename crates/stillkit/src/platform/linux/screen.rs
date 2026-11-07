//! Wayland does not expose global window positions or work areas to ordinary clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Screen.
pub struct Screen {
    /// Work.
    pub work: crate::menu::Edges,
    /// Native display density in dots per inch.
    pub dpi: u32,
}
/// Screen at.
pub fn screen_at(_: i32, _: i32) -> Screen {
    let ((w, h), scale) = super::surface::screen_info();
    Screen {
        work: (0, 0, w, h),
        dpi: 96 * scale,
    }
}
pub(crate) fn animations_enabled() -> bool {
    true
}
