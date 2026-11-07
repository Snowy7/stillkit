//! The element tree's animation: a list whose rows come and go with presence (click a row to
//! remove it), a card that changes size (only its moving edges travel) and a theme switch
//! whose colours spring across. The view only says where things end up.
//!
//!   cargo run -p stillkit --example el_motion
//!   cargo run -p stillkit --example el_motion -- --script add.json --at 0.25 --snap add.png
//!   cargo run -p stillkit --example el_motion -- --reduced

use stillkit::el::{self, Align, App, El, Justify, Paint, Transition, Tx, col, el, row, text, token};
use stillkit::{Kind, Scheme, Style, TextRole, Theme};

pub struct Motion {
    /// The rows' ids, top to bottom.
    pub rows: Vec<u32>,
    pub next: u32,
    pub wide: bool,
    pub dark: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            rows: vec![1, 2, 3],
            next: 4,
            wide: false,
            dark: true,
        }
    }
}

/// The window's own opaque backing, so a theme switch repaints all of it.
const BACKDROP: Paint = Paint::Theme(|t| {
    let c = t.color.surface;
    let under = if t.dark() { 0.0 } else { 1.0 };
    let mix = |v: f32| v * c.a + under * (1.0 - c.a);
    stillkit::Rgba::new(mix(c.r), mix(c.g), mix(c.b), 1.0)
});

fn button(key: &str, label: &str) -> El<Motion> {
    el().key(key)
        .h(token::CONTROL_SMALL)
        .pad_x(12.0)
        .radius(token::RADIUS_CONTROL)
        .fill(token::FILL)
        .justify(Justify::Center)
        .align(Align::Center)
        .child(text(label.to_owned()))
}

fn list_row(id: u32) -> El<Motion> {
    row()
        .key(format!("row-{id}"))
        .h(token::CONTROL)
        .pad_x(10.0)
        .align(Align::Center)
        .radius(token::RADIUS_SMALL)
        .fill(token::CARD)
        .transition(Transition::slide(-24.0, 0.0))
        .label(format!("Row {id}"))
        .on_press(move |s: &mut Motion| s.rows.retain(|&r| r != id))
        .child(text(format!("Row {id}")))
}

pub fn view(s: &Motion) -> El<Motion> {
    let (w, h) = if s.wide { (150.0, 90.0) } else { (90.0, 150.0) };
    col()
        .animate()
        .pad(token::PAD)
        .gap(token::GAP)
        .fill(BACKDROP)
        .child(
            row()
                .gap(8.0)
                .child(button("add", "Add").on_press(|s: &mut Motion| {
                    let at = s.rows.len().min(1);
                    s.rows.insert(at, s.next);
                    s.next += 1;
                }))
                .child(button("resize", "Resize").on_press(|s: &mut Motion| s.wide = !s.wide))
                .child(button("theme", "Theme").on_press_with(|s: &mut Motion, tx: &mut Tx| {
                    s.dark = !s.dark;
                    tx.set_theme(theme(s.dark));
                })),
        )
        .child(
            row()
                .gap(token::GAP)
                .align(Align::Start)
                .child(col().key("list").w(150.0).gap(6.0).children(s.rows.iter().map(|&id| list_row(id))))
                .child(
                    el().key("card")
                        .w(w)
                        .h(h)
                        .pad(10.0)
                        .radius(token::RADIUS_CARD)
                        .fill(token::ACCENT)
                        .shadow()
                        .child(text("Card").font(TextRole::Headline).color(token::ON_ACCENT)),
                ),
        )
}

pub fn theme(dark: bool) -> Theme {
    let scheme = if dark { Scheme::Dark } else { Scheme::Light };
    Theme::new(Style::Liquid, scheme, stillkit::theme::SYSTEM_BLUE)
}

fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Motion",
        size: (340.0, 260.0),
        kind: Kind::App { min: (240, 200) },
        theme: theme(true),
        state: Motion::default(),
        view,
    })
}
