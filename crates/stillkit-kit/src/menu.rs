//! Menu content and point-anchored context menus. The model owns opening and dismissal.
use crate::overlay_surface::{DIM, INK, panel};
use crate::{Icon, Press};
type Dismiss<S> = Arc<dyn Fn(&mut S)>;
use std::sync::Arc;
use stillkit::el::{self, Align, El, Len, Orientation, Overlay, col, icon, row, text};
use stillkit::{Role, TextRole};

/// One actionable menu row. Activation also closes the menu when using `Menu::on_dismiss`.
pub struct MenuItem<S> {
    label: Arc<str>,
    key: Option<Arc<str>>,
    glyph: Option<Icon>,
    hint: Option<Arc<str>>,
    checked: Option<bool>,
    radio: bool,
    disabled: bool,
    press: Option<Press<S>>,
}
impl<S: 'static> MenuItem<S> {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            key: None,
            glyph: None,
            hint: None,
            checked: None,
            radio: false,
            disabled: false,
            press: None,
        }
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn icon(mut self, icon: Icon) -> Self {
        self.glyph = Some(icon);
        self
    }
    pub fn shortcut(mut self, hint: impl Into<Arc<str>>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }
    pub fn radio(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self.radio = true;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_press(mut self, press: impl Fn(&mut S) + 'static) -> Self {
        self.press = Some(Box::new(press));
        self
    }
    pub fn build(self) -> El<S> {
        self.row(None)
    }
    fn row(self, dismiss: Option<Dismiss<S>>) -> El<S> {
        let mut lead = row()
            .w(Len::Theme(|t| t.tokens().menu.gutter))
            .align(Align::Center)
            .justify(el::Justify::Center);
        if self.checked == Some(true) {
            lead = lead.child(if self.radio {
                el::el().size(6.).radius(3.).fill(INK)
            } else {
                icon(Icon::Check).size(12.).color(INK)
            });
        } else if self.checked.is_none()
            && let Some(g) = self.glyph
        {
            lead = lead.child(icon(g).size(16.).color(INK));
        }
        let mut node = row()
            .h(Len::Theme(|t| t.tokens().menu.row))
            .radius(Len::Theme(|t| (t.tokens().radius * 0.6).max(2.)))
            .align(Align::Center)
            .role(if self.radio {
                Role::MenuItemRadio
            } else if self.checked.is_some() {
                Role::MenuItemCheckBox
            } else {
                Role::MenuItem
            })
            .label(self.label.clone())
            .disabled(self.disabled)
            .opacity(if self.disabled { 0.45 } else { 1. })
            .child(lead)
            .child(text(self.label).font(TextRole::Body).color(INK).grow(1.));
        if let Some(c) = self.checked {
            node = node.checked(c);
        }
        if let Some(h) = self.hint {
            node = node
                .description(h.clone())
                .child(text(h).font(TextRole::Footnote).color(DIM).pad_x(12.));
        }
        if let Some(k) = self.key {
            node = node.key(k);
        }
        if let Some(f) = self.press {
            node = node.on_press(move |s| {
                f(s);
                if let Some(d) = &dismiss {
                    d(s);
                }
            });
        }
        node
    }
}
enum Entry<S> {
    Item(MenuItem<S>),
    Separator,
    Header(Arc<str>),
}
/// A menu with old-kit metrics and a vertical, disabled-skipping focus group.
pub struct Menu<S> {
    entries: Vec<Entry<S>>,
    dismiss: Option<Dismiss<S>>,
}
impl<S: 'static> Default for Menu<S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<S: 'static> Menu<S> {
    pub fn new() -> Self {
        Self {
            entries: vec![],
            dismiss: None,
        }
    }
    pub fn item(mut self, item: MenuItem<S>) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
    pub fn section(mut self, title: impl Into<Arc<str>>) -> Self {
        self.entries.push(Entry::Header(title.into()));
        self
    }
    /// Called after an enabled action, and by the runtime on Escape/outside/deactivation.
    pub fn on_dismiss(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.dismiss = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        panel()
            .radius(Len::Theme(|t| t.tokens().radius))
            .min_w(Len::Theme(|t| t.tokens().menu.min_width))
            .pad(Len::Theme(|t| t.tokens().menu.pad))
            .role(Role::Menu)
            .focus_group(Orientation::Vertical)
            .children(self.entries.into_iter().map(|e| {
                match e {
                    Entry::Item(i) => i.row(self.dismiss.clone()),
                    Entry::Separator => col()
                        .h(Len::Theme(|t| t.tokens().menu.separator))
                        .justify(el::Justify::Center)
                        .pad_x(4.)
                        .child(crate::Separator.build()),
                    Entry::Header(h) => text(h.clone())
                        .label(h)
                        .role(Role::Heading)
                        .font(TextRole::Caption)
                        .color(DIM)
                        .pad_x(10.)
                        .pad_y(4.),
                }
            }))
    }
    pub fn overlay(self) -> Overlay<S> {
        let dismiss = self.dismiss.clone();
        let mut overlay = el::menu(self.build());
        if let Some(f) = dismiss {
            overlay = overlay.on_dismiss(move |s| f(s));
        }
        overlay
    }
}
/// Attach a menu opened at a window-space point supplied by `on_context_menu`.
pub struct ContextMenu;
impl ContextMenu {
    pub fn attach<S: 'static>(
        target: El<S>,
        point: Option<(f32, f32)>,
        menu: Menu<S>,
        open: impl Fn(&mut S, f32, f32) + 'static,
    ) -> El<S> {
        target
            .on_context_menu(open)
            .overlay(point.map(|(x, y)| menu.overlay().at(x, y)))
    }
}
