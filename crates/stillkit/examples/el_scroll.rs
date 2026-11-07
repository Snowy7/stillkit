//! A long list in a scroll view under a header that stays put: wheel, Page Up/Down, Tab
//! through the rows (each scrolls into view), and a header button that jumps to the last row
//! through `Tx::scroll_into_view`. Scrolling never runs the view.
//!
//!   cargo run -p stillkit --example el_scroll
//!   cargo run -p stillkit --example el_scroll -- --script s.json --snap scroll.png
//!     with s.json: [{"at": 0.1, "wheel": [160, 200, -360]}]

use stillkit::el::{self, Align, App, Axis, El, Paint, col, row, text, token};
use stillkit::{Kind, Role, Scheme, Style, TextRole, Theme};

pub struct Inbox {
    pub rows: usize,
    pub picked: Option<usize>,
}

/// The selected row's wash.
const PICKED: Paint = Paint::Theme(|t| t.color.accent_wash);

pub fn view(s: &Inbox) -> El<Inbox> {
    let last = s.rows.saturating_sub(1);
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(
            row()
                .key("header")
                .align(Align::Center)
                .gap(token::ITEM)
                .child(text("Inbox").font(TextRole::Title2).grow(1.0))
                .child(
                    row()
                        .key("jump")
                        .role(Role::Button)
                        .h(token::CONTROL_SMALL)
                        .pad_x(12.0)
                        .radius(token::RADIUS_CONTROL)
                        .fill(token::FILL)
                        .align(Align::Center)
                        .on_press_with(move |s: &mut Inbox, tx| {
                            s.picked = Some(last);
                            tx.scroll_into_view(format!("row-{last}"));
                        })
                        .child(text("Last")),
                ),
        )
        .child(
            col()
                .key("list")
                .label("Messages")
                .scroll(Axis::Vertical)
                .grow(1.0)
                .fill(token::CARD)
                .radius(token::RADIUS_CARD)
                .pad(4.0)
                .gap(2.0)
                .children((0..s.rows).map(|i| {
                    let picked = s.picked == Some(i);
                    row()
                        .key(format!("row-{i}"))
                        .h(token::ROW_SINGLE)
                        .pad_x(12.0)
                        .radius(8.0)
                        .align(Align::Center)
                        .fill(if picked { PICKED } else { Paint::Rgba(stillkit::Rgba::TRANSPARENT) })
                        .selected(picked)
                        .on_press(move |s: &mut Inbox| s.picked = Some(i))
                        .child(text(format!("Message {}", i + 1)).grow(1.0))
                        .child(text(format!("{}:{:02}", 9 + i / 60, i % 60)).color(token::SECONDARY))
                })),
        )
}

pub fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Inbox",
        size: (320.0, 400.0),
        kind: Kind::App { min: (240, 200) },
        theme: theme(),
        state: Inbox { rows: 60, picked: None },
        view,
    })
}
