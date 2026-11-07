//! The element tree's hello: a switch flipping a label, drawn with elements only.
//!
//!   cargo run -p stillkit --example el_hello
//!   cargo run -p stillkit --example el_hello -- --snap hello.png --scale 2
//!   cargo run -p stillkit --example el_hello -- --tree tree.json

use stillkit::el::{self, Align, App, El, Justify, col, el, text, token};
use stillkit::{Kind, Role, Scheme, Style, TextRole, Theme};

pub struct Hello {
    pub on: bool,
}

pub fn view(s: &Hello) -> El<Hello> {
    let knob = 20.0;
    col()
        .align(Align::Center)
        .justify(Justify::Center)
        .gap(token::GAP)
        .child(
            text(if s.on { "The light is on" } else { "The light is off" })
                .font(TextRole::Title2)
                .key("label"),
        )
        .child(
            el().key("switch")
                .role(Role::Switch)
                .label("Light")
                .checked(s.on)
                .w(40.0)
                .h(24.0)
                .radius(12.0)
                .fill(if s.on { token::ACCENT } else { token::FILL })
                .on_press(|s: &mut Hello| s.on = !s.on)
                .child(
                    el().key("knob")
                        .size(knob)
                        .radius(knob / 2.0)
                        .fill(token::KNOB)
                        .absolute(if s.on { 18.0 } else { 2.0 }, 2.0),
                ),
        )
}

pub fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Hello",
        size: (320.0, 180.0),
        kind: Kind::App { min: (200, 120) },
        theme: theme(),
        state: Hello { on: false },
        view,
    })
}
