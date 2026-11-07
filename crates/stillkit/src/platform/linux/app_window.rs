//! Client-side chrome actions, set with `Surface::set_chrome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chrome {
    Caption,
    Minimize,
    Maximize,
    Close,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Arrow,
    Text,
    Hand,
    Move,
    Resize,
    ResizeHorizontal,
}
pub fn set_modal_hook(_: Option<Box<dyn FnMut()>>) {}
