//! A controlled select: opening and selection live in the application model.
use crate::overlay_surface::{DIM, INK};
use crate::{Change, Icon, Menu, MenuItem};
use std::sync::Arc;
use stillkit::el::{Align, El, Handled, KeyCode, icon, row, text, token};
use stillkit::{Role, TextRole};
/// A trigger and a match-width radio menu. Down opens too, because el has no modifier data.
pub struct Select<S> {
    label: Arc<str>,
    options: Vec<Arc<str>>,
    selected: usize,
    open: bool,
    key: Arc<str>,
    disabled: bool,
    toggle: Change<S, bool>,
    change: Change<S, usize>,
}
impl<S: 'static> Select<S> {
    pub fn new(
        label: impl Into<Arc<str>>,
        options: impl IntoIterator<Item = impl Into<Arc<str>>>,
        selected: usize,
        open: bool,
        on_open: impl Fn(&mut S, bool) + 'static,
        on_change: impl Fn(&mut S, usize) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            open,
            key: "select".into(),
            disabled: false,
            toggle: Arc::new(on_open),
            change: Arc::new(on_change),
        }
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = key.into();
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn build(self) -> El<S> {
        let mut menu = Menu::new();
        for (i, name) in self.options.iter().enumerate() {
            let change = self.change.clone();
            menu = menu.item(
                MenuItem::new(name.clone())
                    .key(format!("{}/option-{i}", self.key))
                    .radio(i == self.selected)
                    .on_press(move |s| {
                        change(s, i);
                    }),
            );
        }
        let dismiss = self.toggle.clone();
        let overlay = menu
            .on_dismiss(move |s| dismiss(s, false))
            .overlay()
            .match_width()
            .open(self.open && !self.disabled);
        let value = self.options.get(self.selected).cloned().unwrap_or_default();
        let (press, down) = (self.toggle.clone(), self.toggle);
        let open = self.open;
        let disabled = self.disabled;
        row()
            .key(self.key)
            .role(Role::ComboBox)
            .label(self.label)
            .value(value.clone())
            .expanded(open && !self.disabled)
            .h(token::CONTROL)
            .min_w(180.)
            .pad_x(12.)
            .gap(12.)
            .align(Align::Center)
            .radius(token::RADIUS_CONTROL)
            .fill(token::FILL)
            .disabled(self.disabled)
            .opacity(if self.disabled { 0.45 } else { 1. })
            .child(text(value).font(TextRole::Body).color(INK).grow(1.))
            .child(icon(Icon::ChevronDown).size(12.).color(DIM))
            .on_press(move |s| press(s, !open))
            .on_key(move |s, key, _| {
                if key == KeyCode::DOWN && !open && !disabled {
                    down(s, true);
                    Handled::Yes
                } else {
                    Handled::No
                }
            })
            .overlay(overlay)
    }
}
