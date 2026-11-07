//! Input methods for `el`'s text fields, through the view's NSTextInputClient: while a field
//! has focus, the view knows its caret (the candidate window opens there); the marked text
//! arrives as `Event::Preedit` (read it with [`preedit`]) and is drawn inline by `el`, commits
//! as `Event::Char`s. Focus leaving the field discards a composition in progress.
use super::surface::{Event, SurfaceId};
use crate::Rect;
use objc2::DefinedClass;
use std::cell::RefCell;

thread_local! {
    /// The composition now: its text and caret (bytes into it).
    static PREEDIT: RefCell<(String, usize)> = const { RefCell::new((String::new(), 0)) };
}

pub(super) fn set_preedit(text: String, cursor: usize) {
    PREEDIT.with_borrow_mut(|p| *p = (text, cursor));
}

/// The composition now: its text and its caret (bytes into it).
pub fn preedit() -> (String, usize) {
    PREEDIT.with_borrow(Clone::clone)
}

/// The focused field of surface `id` and its caret (device px), or none: the input context
/// follows. Tells AppKit only what changed.
pub fn follow(id: SurfaceId, caret: Option<Rect>) {
    let Some(view) = super::surface::view(id) else { return };
    let was = view.ivars().caret.replace(caret);
    if was == caret {
        return;
    }
    let context = view.inputContext();
    if caret.is_none() && !view.ivars().marked.take().is_empty() {
        if let Some(context) = &context {
            context.discardMarkedText();
        }
        set_preedit(String::new(), 0);
        super::pump::push(id, Event::Preedit);
    }
    if let Some(context) = context {
        context.invalidateCharacterCoordinates();
    }
}
