# Components

Kit builders return ordinary El<State> nodes. Feed current checked values, selection indices and slider fractions back from state each view. Add layout after build(). Keys expose parts such as volume/track and mode/option-1.

Start with Button/IconButton, Switch/Checkbox/RadioGroup, Slider/Segmented and Card/ListRow/Tile. Tabs, Disclosure, Sidebar, Toolbar, Table and Tree build larger screens. Progress, Ring, Badge, Avatar, CalendarMonth and Chart display data. Select, Menu, Dialog, Popover, Tooltip and Toaster handle transient content. Use el text_input/text_area for editing; TextFieldFrame alone is presentation.

Gallery sheets: [controls](../media/kit/overview.png), [layout and data](../media/kit/layout-oneui-dark.png), [overlays](../media/kit/overlays-overview.png). See the [kit reference](../../crates/stillkit-kit/README.md) for interaction limitations.

```rust
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
```

[Executable source](../../crates/stillkit-kit/examples/guide_components.rs).


[All guides](../../README.md#guides) | [Element reference](../el.md)
