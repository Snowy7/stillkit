# State and events

The view borrows state. Handlers mutate it between frames; a queued press causes the view to run for the changed state. Hover and focus repaint the retained tree without rerunning the view.

Tx applies frame policy: instant skips motion, set_theme changes the theme, modifiers reads held keys and scroll_into_view requests a keyed item. A key handler returns Handled::Yes only for keys it consumes. Focus groups provide one Tab stop and arrow navigation; select_follows_focus also activates each newly focused member. Enter/Space activate press handlers. Stable keys retain focus across rebuilds.

```rust
use stillkit::el::{self, App, El, Handled, Tx, col, row, text, token};
use stillkit::{Kind, Scheme, Theme};
#[derive(Default)]
struct State {
    count: u32,
    dark: bool,
}
fn view(s: &State) -> El<State> {
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .on_key(|s: &mut State, key, tx| {
            if key == el::KeyCode(0x1B) {
                s.count = 0;
                tx.instant();
                Handled::Yes
            } else {
                Handled::No
            }
        })
        .child(text(format!("{} clicks (Escape resets)", s.count)))
        .child(
            row()
                .focus_group(el::Orientation::Horizontal)
                .child(
                    text("Add")
                        .key("add")
                        .on_press(|s: &mut State| s.count += 1),
                )
                .child(
                    text("Theme")
                        .key("theme")
                        .on_press_with(|s: &mut State, tx: &mut Tx| {
                            s.dark = !s.dark;
                            tx.set_theme(Theme::solid(
                                if s.dark { Scheme::Dark } else { Scheme::Light },
                                stillkit::theme::SYSTEM_BLUE,
                            ));
                        }),
                ),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State {
            dark: true,
            ..Default::default()
        },
        view,
    })
}
```

[Executable source](../../crates/stillkit/examples/guide_state_and_events.rs).


[All guides](../../README.md#guides) | [Element reference](../el.md)
