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
    unimplemented!("monitor query")
}
pub(crate) fn animations_enabled() -> bool {
    unimplemented!("system animation preference")
}
