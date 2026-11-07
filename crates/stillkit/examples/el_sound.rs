//! The plan's "Sound" panel on the element tree, its controls today's kit through `paint`:
//! a title, a volume slider (drag it) and a mute switch.
//!
//!   cargo run -p stillkit --example el_sound [-- --snap sound.png --scale 2]
//!
//! `tests/el_sound.rs` checks it damages exactly what the hand-placed `Frame` version does.

use stillkit::el::{self, Align, App, El, col, paint, row, text, token};
use stillkit::icons::Icon;
use stillkit::{Kind, Scheme, Style, TextRole, Theme};

pub struct Sound {
    pub level: f32,
    pub muted: bool,
}

pub fn view(s: &Sound) -> El<Sound> {
    let (level, muted) = (s.level, s.muted);
    col()
        .pad(token::PAD)
        .gap(8.0)
        .child(text("Sound").font(TextRole::Title2).key("title"))
        .child(
            paint(move |f, rect, id| f.slider(id, rect, level, "Volume", Some(Icon::Volume(2))))
                .key("volume")
                .w_full()
                .h(token::CONTROL)
                .on_drag(|s: &mut Sound, x, _| s.level = x),
        )
        .child(
            row()
                .h(token::ITEM)
                .align(Align::Center)
                .child(text("Mute").grow(1.0))
                .child(
                    paint(move |f, rect, id| {
                        f.switch(id, rect.right, rect.center().y, muted, "Mute", true);
                    })
                    .key("mute")
                    .w(40.0)
                    .h(24.0)
                    .on_press(|s: &mut Sound| s.muted = !s.muted),
                ),
        )
}

pub fn theme() -> Theme {
    Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Sound",
        size: (340.0, 160.0),
        kind: Kind::App { min: (240, 140) },
        theme: theme(),
        state: Sound {
            level: 0.4,
            muted: false,
        },
        view,
    })
}
