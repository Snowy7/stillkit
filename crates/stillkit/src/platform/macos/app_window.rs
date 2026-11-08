//! The app-window kind ([`Kind::App`](crate::Kind::App)): an NSWindow whose content view fills
//! the whole frame (a transparent title bar), so the app draws its own title area while
//! AppKit keeps the traffic lights. [`Surface::set_chrome`](crate::Surface::set_chrome) marks
//! the caption (dragged with `performWindowDragWithEvent`, double-click zooms) and any
//! caption buttons the app draws itself.
//!
//! AppKit runs a tracking loop while the user drags the frame, and the app's own loop stops;
//! [`set_modal_hook`] runs on each size step then, so the app can render during it.
use std::cell::RefCell;

/// What a rectangle of the app's own title area is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chrome {
    /// Drags the window (and double-click zooms), wherever no hit item is under the pointer.
    Caption,
    /// Minimize.
    Minimize,
    /// Maximize.
    Maximize,
    /// Close.
    Close,
}

/// The pointer shape over the client area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

thread_local! {
    static MODAL: RefCell<Option<Box<dyn FnMut()>>> = RefCell::new(None);
}

/// Runs `hook` on every size step of a live resize (AppKit's tracking loop, when the app's
/// own loop can't run). `None` removes it.
pub fn set_modal_hook(hook: Option<Box<dyn FnMut()>>) {
    MODAL.with_borrow_mut(|m| *m = hook);
}

/// Taken out while it runs, so a nested resize can't run it twice.
pub(super) fn run_modal_hook() {
    let Some(mut hook) = MODAL.with_borrow_mut(Option::take) else { return };
    hook();
    MODAL.with_borrow_mut(|m| {
        if m.is_none() {
            *m = Some(hook);
        }
    });
}
