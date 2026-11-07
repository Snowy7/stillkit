//! Model-owned notifications with one-shot deadlines; no kit ticker or background task.
use crate::overlay_surface::{INK, ShadowLevel, panel};
use crate::{Button, ButtonKind, Icon};
use std::{sync::Arc, time::Duration};
use stillkit::Role;
use stillkit::el::{self, Align, El, Overlay, Transition, icon, row, text, token};
/// A polite status announcement with optional action and explicit model-owned timeout.
pub struct Toast<S> {
    key: Arc<str>,
    message: Arc<str>,
    glyph: Icon,
    action: Option<Button<S>>,
}
impl<S: 'static> Toast<S> {
    /// Use a unique key for every toast, even when messages repeat.
    pub fn new(key: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
            glyph: Icon::Check,
            action: None,
        }
    }
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = glyph;
        self
    }
    pub fn action(mut self, label: impl Into<Arc<str>>, action: impl Fn(&mut S) + 'static) -> Self {
        self.action = Some(Button::new(label).kind(ButtonKind::Plain).on_press(action));
        self
    }
    pub fn build(self) -> El<S> {
        let mut content = row()
            .gap(10.)
            .align(Align::Center)
            .child(icon(self.glyph).size(18.).color(INK))
            .child(text(self.message.clone()).color(INK).wrap(3).grow(1.));
        if let Some(action) = self.action {
            content = content.child(action.build());
        }
        panel(ShadowLevel::Toast)
            .key(self.key)
            .role(Role::Status)
            .label(self.message)
            .max_w(400.)
            .pad_x(14.)
            .pad_y(8.)
            .transition(Transition::slide(0., 12.))
            .child(content)
    }
    /// Remove this keyed toast from the model at the deadline.
    pub fn timeout(self, after: Duration, dismiss: impl Fn(&mut S) + 'static) -> El<S> {
        self.build().on_timeout(after, dismiss)
    }
}
/// A bottom-right stack. Each child owns its deadline and unique key.
pub struct Toaster<S> {
    stack: El<S>,
}
impl<S: 'static> Default for Toaster<S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<S: 'static> Toaster<S> {
    pub fn new() -> Self {
        Self {
            stack: el::col()
                .gap(token::GAP)
                .align(Align::End)
                .motion(token::MOTION_OPEN),
        }
    }
    pub fn child(mut self, toast: El<S>) -> Self {
        self.stack = self.stack.child(toast);
        self
    }
    pub fn build(self) -> Overlay<S> {
        el::toasts(self.stack)
    }
}
