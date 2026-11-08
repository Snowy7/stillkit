//! Client-side chrome actions, set with `Surface::set_chrome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Chrome.
pub enum Chrome {
    /// Caption.
    Caption,
    /// Minimize.
    Minimize,
    /// Maximize.
    Maximize,
    /// Close.
    Close,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Cursor.
pub enum Cursor {
    #[default]
    /// Arrow.
    Arrow,
    /// Text.
    Text,
    /// Hand.
    Hand,
    /// Move.
    Move,
    /// Resize.
    Resize,
    /// Resize horizontal.
    ResizeHorizontal,
    /// Resize vertically (a horizontal splitter).
    ResizeVertical,
    /// An open hand: something can be dragged.
    Grab,
    /// A closed hand: something is being dragged.
    Grabbing,
    /// A crosshair: precise picking.
    Crosshair,
    /// The action is not allowed here (a refused drop).
    NotAllowed,
    /// A drop here copies.
    Copy,
}
/// Sets modal hook.
pub fn set_modal_hook(_: Option<Box<dyn FnMut()>>) {}
