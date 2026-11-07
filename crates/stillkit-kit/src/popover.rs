//! Elevated, composable popover content; attach with `el::popover(content)`.
use crate::overlay_surface::{DIM, INK, ShadowLevel, panel};
use std::sync::Arc;
use stillkit::el::{El, col, text, token};
use stillkit::{Role, TextRole};
/// A themed title, body and optional caller-owned action row.
pub struct Popover<S> {
    title: Option<Arc<str>>,
    body: El<S>,
    actions: Option<El<S>>,
}
impl<S: 'static> Popover<S> {
    pub fn new(body: El<S>) -> Self {
        Self {
            title: None,
            body,
            actions: None,
        }
    }
    pub fn title(mut self, title: impl Into<Arc<str>>) -> Self {
        self.title = Some(title.into());
        self
    }
    pub fn actions(mut self, actions: El<S>) -> Self {
        self.actions = Some(actions);
        self
    }
    pub fn build(self) -> El<S> {
        let mut p = panel(ShadowLevel::Popover)
            .pad(token::PAD)
            .gap(token::GAP)
            .min_w(240.)
            .max_w(420.);
        if let Some(title) = self.title {
            p = p
                .label(title.clone())
                .child(text(title).font(TextRole::Headline).color(INK));
        }
        p = p.child(col().color(DIM).child(self.body));
        if let Some(actions) = self.actions {
            p = p.child(actions);
        }
        p.role(Role::Dialog)
    }
}
