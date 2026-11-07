#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub work: crate::menu::Edges,
    pub dpi: u32,
}
pub fn screen_at(_: i32, _: i32) -> Screen {
    unimplemented!("monitor query")
}
pub(crate) fn animations_enabled() -> bool {
    unimplemented!("system animation preference")
}
