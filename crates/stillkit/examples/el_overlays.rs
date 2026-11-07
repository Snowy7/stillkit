//! Anchored overlays: a popover under a button, a context menu on right-click (or the Menu
//! key), a tooltip, a confirmation dialog and a stack of toasts that time out.
//!
//!   cargo run -p stillkit --example el_overlays
//!   cargo run -p stillkit --example el_overlays -- --snap overlays.png --scale 2

use std::time::Duration;

use stillkit::el::{
    self, Align, App, El, Justify, Transition, col, dialog, menu, popover, row, text, toasts, token,
};
use stillkit::{Kind, Role, Scheme, Style, TextRole, Theme};

#[derive(Default)]
pub struct Demo {
    pub share: bool,
    /// Where the context menu is open, design px.
    pub menu: Option<(f32, f32)>,
    pub confirm: bool,
    pub toasts: Vec<(u32, &'static str)>,
    pub next: u32,
    pub deleted: u32,
}

impl Demo {
    pub fn toast(&mut self, message: &'static str) {
        self.next += 1;
        self.toasts.push((self.next, message));
    }
}

/// How long a toast stays.
pub const TOAST: Duration = Duration::from_secs(3);

fn button(key: &'static str, label: &'static str) -> El<Demo> {
    row()
        .key(key)
        .h(token::CONTROL)
        .pad_x(14.0)
        .radius(token::RADIUS_CONTROL)
        .fill(token::FILL)
        .align(Align::Center)
        .child(text(label))
}

fn item(key: &'static str, label: &'static str, on: impl Fn(&mut Demo) + 'static) -> El<Demo> {
    row()
        .key(key)
        .role(Role::MenuItem)
        .h(28.0)
        .pad_x(10.0)
        .radius(6.0)
        .align(Align::Center)
        .child(text(label))
        .on_press(move |s: &mut Demo| {
            s.menu = None;
            on(s);
        })
}

fn card() -> El<Demo> {
    col()
        .pad(token::PAD)
        .gap(8.0)
        .fill(token::ELEVATED)
        .radius(token::RADIUS_CARD)
        .shadow()
}

pub fn view(s: &Demo) -> El<Demo> {
    let share = popover(
        card()
            .key("share-panel")
            .w(220.0)
            .child(text("Share link").font(TextRole::Headline))
            .child(text("Anyone with the link can view it.").font(TextRole::Footnote).wrap(2))
            .child(button("share-copy", "Copy link").on_press(|s: &mut Demo| {
                s.share = false;
                s.toast("Link copied");
            })),
    )
    .open(s.share)
    .on_dismiss(|s: &mut Demo| s.share = false);

    let confirm = dialog(
        card()
            .key("confirm-panel")
            .w(300.0)
            .gap(token::GAP)
            .child(text("Delete this item?").font(TextRole::Title3))
            .child(text("It goes for good.").font(TextRole::Footnote))
            .child(
                row()
                    .gap(8.0)
                    .justify(Justify::End)
                    .child(button("cancel", "Cancel").on_press(|s: &mut Demo| s.confirm = false))
                    .child(button("ok", "Delete").fill(token::ACCENT).color(token::ON_ACCENT).on_press(
                        |s: &mut Demo| {
                            s.confirm = false;
                            s.deleted += 1;
                            s.toast("Deleted");
                        },
                    )),
            ),
    )
    .open(s.confirm)
    .on_dismiss(|s: &mut Demo| s.confirm = false);

    let context = s.menu.map(|(x, y)| {
        menu(
            col()
                .key("menu")
                .w(180.0)
                .pad(4.0)
                .fill(token::ELEVATED)
                .radius(token::RADIUS_CARD)
                .shadow()
                .child(item("cut", "Cut", |s| s.toast("Cut")))
                .child(item("copy-item", "Copy", |s| s.toast("Copied")))
                .child(item("paste", "Paste", |s| s.toast("Pasted"))),
        )
        .at(x, y)
        .on_dismiss(|s: &mut Demo| s.menu = None)
    });

    let stack = col().key("toasts").gap(8.0).align(Align::End).animate().children(s.toasts.iter().map(
        |&(id, message)| {
            row()
                .key(id.to_string())
                .role(Role::Status)
                .label(message)
                .h(36.0)
                .pad_x(14.0)
                .radius(token::RADIUS_CARD)
                .align(Align::Center)
                .fill(token::ELEVATED)
                .shadow()
                .transition(Transition::slide(0.0, 16.0))
                .child(text(message))
                .on_timeout(TOAST, move |s: &mut Demo| s.toasts.retain(|t| t.0 != id))
        },
    ));

    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(
            row()
                .gap(8.0)
                .child(button("share", "Share").on_press(|s: &mut Demo| s.share = !s.share).overlay(share))
                .child(
                    button("copy", "Copy")
                        .tooltip("Copy the link")
                        .on_press(|s: &mut Demo| s.toast("Link copied")),
                )
                .child(button("delete", "Delete…").on_press(|s: &mut Demo| s.confirm = true).overlay(confirm)),
        )
        .child(
            col()
                .key("canvas")
                .grow(1.0)
                .focusable()
                .label("Canvas")
                .radius(token::RADIUS_CARD)
                .fill(token::WELL)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(text(format!("Right-click for a menu · {} deleted", s.deleted)).color(token::SECONDARY))
                .on_context_menu(|s: &mut Demo, x, y| s.menu = Some((x, y)))
                .overlay(context),
        )
        .overlay(toasts(stack))
}

pub fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

#[allow(dead_code)]
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Overlays",
        size: (480.0, 320.0),
        kind: Kind::App { min: (320, 240) },
        theme: theme(),
        state: Demo::default(),
        view,
    })
}
