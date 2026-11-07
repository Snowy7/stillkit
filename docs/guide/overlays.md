# Overlays

Declare overlays on their anchor or root; keep open state in your model. Dialog traps keyboard focus, Escape/backdrop dismisses it and focus returns to the trigger. Confirmation runs before dismissal. Toaster handles layout while your app owns the Toast values; timeout removes one through its handler.

Popover/Menu attach to triggers. Select combines a trigger with a controlled menu. Tooltip::attach provides delayed help. Windows can put overflowing menus in popup surfaces; do not assume every backend supports escaping the window.

```rust
use std::time::Duration;
use stillkit::el::{self, App, El, col};
use stillkit::{Kind, Scheme, Theme};
use stillkit_kit::{Button, Dialog, Toast, Toaster};
#[derive(Default)]
struct State {
    dialog: bool,
    saved: bool,
}
fn view(s: &State) -> El<State> {
    let mut toasts = Toaster::new();
    if s.saved {
        toasts = toasts.child(
            Toast::new("saved", "Settings saved")
                .timeout(Duration::from_secs(3), |s: &mut State| s.saved = false),
        );
    }
    col()
        .pad(24.)
        .child(
            Button::new("Save")
                .key("save")
                .on_press(|s: &mut State| s.dialog = true)
                .build(),
        )
        .overlay(
            Dialog::new(
                "Save settings?",
                "Apply your preferences.",
                |s: &mut State| s.dialog = false,
            )
            .confirm("Save", |s| s.saved = true)
            .build()
            .open(s.dialog),
        )
        .overlay(toasts.build())
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Overlays",
        size: (500., 360.),
        kind: Kind::App { min: (400, 300) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State::default(),
        view,
    })
}
```

[Executable source](../../crates/stillkit-kit/examples/guide_overlays.rs).


[All guides](../../README.md#guides) ? [Element reference](../el.md)
