//! Modal confirmation panels. Runtime owns Escape, backdrop dismissal and focus return.
use crate::overlay_surface::{DIM, INK, panel};
use crate::{Button, ButtonKind};
use std::sync::Arc;
use stillkit::el::{self, Justify, Overlay, row, text, token};
use stillkit::{Role, TextRole};
/// A modal with focus starting on Cancel, the safest action.
pub struct Dialog<S> {
    title: Arc<str>,
    body: Arc<str>,
    confirm: Arc<str>,
    cancel: Arc<str>,
    destructive: bool,
    dismiss: Arc<dyn Fn(&mut S)>,
    accept: Arc<dyn Fn(&mut S)>,
    key: Arc<str>,
    alert: bool,
}
impl<S: 'static> Dialog<S> {
    pub fn new(
        title: impl Into<Arc<str>>,
        body: impl Into<Arc<str>>,
        on_dismiss: impl Fn(&mut S) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            confirm: "OK".into(),
            cancel: "Cancel".into(),
            destructive: false,
            dismiss: Arc::new(on_dismiss),
            accept: Arc::new(|_| {}),
            key: "dialog".into(),
            alert: false,
        }
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = key.into();
        self
    }
    pub fn confirm(
        mut self,
        label: impl Into<Arc<str>>,
        action: impl Fn(&mut S) + 'static,
    ) -> Self {
        self.confirm = label.into();
        self.accept = Arc::new(action);
        self
    }
    pub fn cancel_label(mut self, label: impl Into<Arc<str>>) -> Self {
        self.cancel = label.into();
        self
    }
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }
    pub fn build(self) -> Overlay<S> {
        let (cancel, close, dismiss) = (self.dismiss.clone(), self.dismiss.clone(), self.dismiss);
        let accept = self.accept;
        let content = panel()
            .key(self.key.clone())
            .w(360.)
            .pad(token::PAD)
            .gap(token::GAP)
            .role(if self.alert {
                Role::AlertDialog
            } else {
                Role::Dialog
            })
            .label(self.title.clone())
            .description(self.body.clone())
            .child(text(self.title).font(TextRole::Title3).color(INK))
            .child(text(self.body).wrap(6).color(DIM))
            .child(
                row()
                    .gap(8.)
                    .justify(Justify::End)
                    .child(
                        Button::new(self.cancel)
                            .key(format!("{}/cancel", self.key))
                            .kind(ButtonKind::Gray)
                            .on_press(move |s| cancel(s))
                            .build()
                            .selected(true),
                    )
                    .child(
                        Button::new(self.confirm)
                            .key(format!("{}/confirm", self.key))
                            .kind(if self.destructive {
                                ButtonKind::Destructive
                            } else {
                                ButtonKind::Filled
                            })
                            .on_press(move |s| {
                                accept(s);
                                close(s);
                            })
                            .build(),
                    ),
            );
        el::dialog(content).on_dismiss(move |s| dismiss(s))
    }
}
/// A destructive alert dialog, otherwise sharing Dialog's safe focus and dismissal.
pub struct AlertDialog<S>(Dialog<S>);
impl<S: 'static> AlertDialog<S> {
    pub fn new(
        title: impl Into<Arc<str>>,
        body: impl Into<Arc<str>>,
        on_dismiss: impl Fn(&mut S) + 'static,
    ) -> Self {
        let mut d = Dialog::new(title, body, on_dismiss).destructive(true);
        d.alert = true;
        Self(d)
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.0 = self.0.key(key);
        self
    }
    pub fn confirm(
        mut self,
        label: impl Into<Arc<str>>,
        action: impl Fn(&mut S) + 'static,
    ) -> Self {
        self.0 = self.0.confirm(label, action);
        self
    }
    pub fn build(self) -> Overlay<S> {
        self.0.build()
    }
}
