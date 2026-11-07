use stillkit::el::{self, App, El, col, token};
use stillkit::{Kind, Scheme, Theme};
use stillkit_kit::{Button, Card, Slider, Switch};
#[derive(Default)]
struct State {
    enabled: bool,
    level: f32,
}
fn view(s: &State) -> El<State> {
    col().pad(token::PAD).child(
        Card::new()
            .child(
                Switch::new("Sync", s.enabled)
                    .key("sync")
                    .on_toggle(|s: &mut State| s.enabled = !s.enabled)
                    .build(),
            )
            .child(
                Slider::new("Volume", s.level)
                    .key("volume")
                    .on_change(|s: &mut State, v| s.level = v)
                    .build(),
            )
            .child(
                Button::new("Reset")
                    .key("reset")
                    .on_press(|s: &mut State| *s = State::default())
                    .build(),
            )
            .build(),
    )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Components",
        size: (400., 300.),
        kind: Kind::App { min: (300, 220) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State {
            level: 0.5,
            ..Default::default()
        },
        view,
    })
}
