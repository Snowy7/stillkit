//! Basic windowless text storage and keyboard editing. Wayland IME is not implemented yet.
use super::surface::{Event, Surface, SurfaceId};
use crate::{Rect, Rgba, TextStyle};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
#[derive(Debug)]
struct State {
    window: SurfaceId,
    text: String,
    style: TextStyle,
    color: Rgba,
    rect: Rect,
    revision: u64,
    multiline: bool,
}
/// A shared field value; retain it in `Op::TextField` and focus it for basic keyboard input.
#[derive(Debug, Clone)]
pub struct TextField(Rc<RefCell<State>>);
impl PartialEq for TextField {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
thread_local! {static FOCUSED:RefCell<Weak<RefCell<State>>>=const{RefCell::new(Weak::new())};}
impl TextField {
    /// Creates a new instance.
    pub fn new(
        window: &Surface,
        rect: Rect,
        multiline: bool,
        size: f32,
        color: Rgba,
    ) -> anyhow::Result<Self> {
        Ok(Self(Rc::new(RefCell::new(State {
            window: window.id(),
            text: String::new(),
            style: TextStyle::new("Inter", size),
            color,
            rect,
            revision: 0,
            multiline,
        }))))
    }
    /// Revision.
    pub fn revision(&self) -> u64 {
        self.0.borrow().revision
    }
    /// Text content.
    pub fn text(&self) -> anyhow::Result<String> {
        Ok(self.0.borrow().text.clone())
    }
    /// Sets text.
    pub fn set_text(&self, text: &str) -> anyhow::Result<()> {
        let mut s = self.0.borrow_mut();
        if s.text != text {
            s.text = text.into();
            s.revision += 1;
            super::surface::queue_for(s.window, Event::TextChanged);
        }
        Ok(())
    }
    /// Sets style.
    pub fn set_style(&self, style: &TextStyle, color: Rgba) {
        let mut s = self.0.borrow_mut();
        s.style = style.clone();
        s.color = color;
        s.revision += 1;
    }
    /// Sets rect.
    pub fn set_rect(&self, rect: Rect) {
        self.0.borrow_mut().rect = rect;
    }
    /// The hit with keyboard focus.
    pub fn focus(&self) {
        FOCUSED.with_borrow_mut(|f| *f = Rc::downgrade(&self.0));
    }
    /// Blur.
    pub fn blur(&self) {
        FOCUSED.with_borrow_mut(|f| {
            if f.ptr_eq(&Rc::downgrade(&self.0)) {
                *f = Weak::new();
            }
        });
    }
    /// Caret.
    pub fn caret(&self) -> Option<Rect> {
        None
    }
    /// Multiline.
    pub fn multiline(&self) -> bool {
        self.0.borrow().multiline
    }
    pub(crate) fn op(&self, rect: Rect) -> crate::Op {
        let s = self.0.borrow();
        crate::Op::Text {
            rect,
            text: s.text.as_str().into(),
            style: s.style.clone(),
            color: s.color,
        }
    }
}
pub(crate) fn key(window: SurfaceId, symbol: u32, utf8: &str) -> Option<Event> {
    FOCUSED.with_borrow(|f| {
        let field = f.upgrade()?;
        let mut s = field.borrow_mut();
        if s.window != window {
            return None;
        }
        match symbol {
            0xff08 => {
                s.text.pop();
            }
            0xff0d => {
                if !s.multiline {
                    return Some(Event::TextSubmit);
                }
                s.text.push('\n');
            }
            _ => {
                if utf8.is_empty() || utf8.chars().any(char::is_control) {
                    return None;
                }
                s.text.push_str(utf8);
            }
        }
        s.revision += 1;
        Some(Event::TextChanged)
    })
}
impl crate::platform::TextInput for TextField {
    fn text(&self) -> anyhow::Result<String> {
        self.text()
    }
    fn set_text(&self, text: &str) -> anyhow::Result<()> {
        self.set_text(text)
    }
    fn focus(&self) {
        self.focus();
    }
    fn blur(&self) {
        self.blur();
    }
    fn caret(&self) -> Option<Rect> {
        self.caret()
    }
}
