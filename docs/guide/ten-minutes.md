# stillkit in 10 minutes

Build a device-settings window with a sidebar, an editable form, a download list
and a confirmation dialog. Start with the dependency and AccessKit patch from
[getting started](getting-started.md). Use `stillkit-kit` for controls; the optional
`stillkit-playground` crate supplies the examples below in this workspace.

## Run the complete app

```sh
cargo run -p stillkit-playground --example ten_minutes
cargo run -p stillkit-playground --example ten_minutes -- --snap settings.png --tree settings.json
```

The full source below is also the [runnable example](../../crates/stillkit-playground/examples/ten_minutes.rs).
It is included in the playground crate's documentation so `cargo test --doc`
compiles it. The model owns editable values; handlers change that model between
frames. Stable keys let the same app run headlessly with the existing
[agent tooling](agent-tooling.md).

```rust,no_run
//! A complete small settings window. Headless CLI: --snap, --tree and --script.
use stillkit::el::{self, App, El, col, row, text, token};
use stillkit::{Kind, Scheme, Theme};
use stillkit_kit::{Button, Card, Dialog, Icon, ListRow, Sidebar, Switch, TextField};

#[derive(Default)]
struct Settings {
    name: String,
    sync: bool,
    page: usize,
    confirm: bool,
    saved: bool,
}

fn view(s: &Settings) -> El<Settings> {
    let sidebar = Sidebar::new("Settings")
        .item(
            "general",
            "General",
            Icon::Check,
            s.page == 0,
            |s: &mut Settings| s.page = 0,
        )
        .item(
            "downloads",
            "Downloads",
            Icon::Check,
            s.page == 1,
            |s: &mut Settings| s.page = 1,
        )
        .build()
        .w(180.);
    let form = Card::new()
        .child(
            TextField::new("Display name")
                .key("name")
                .value(s.name.clone())
                .on_input(|s: &mut Settings, value| s.name = value.into())
                .build(),
        )
        .child(
            row().child(text("Sync settings").grow(1.)).child(
                Switch::new("Sync settings", s.sync)
                    .key("sync")
                    .on_toggle(|s: &mut Settings| s.sync = !s.sync)
                    .build(),
            ),
        )
        .build();
    let mut list = col().gap(token::ITEM);
    for (key, title) in [
        ("notes", "Notes.txt"),
        ("photo", "Photo.png"),
        ("budget", "Budget.csv"),
    ] {
        list = list.child(
            ListRow::new(title)
                .key(key)
                .subtitle("Available offline")
                .build(),
        );
    }
    let body = if s.page == 0 {
        form
    } else {
        Card::new().child(list).build()
    };
    let mut save = Button::new(if s.saved { "Saved" } else { "Save changes" })
        .key("save")
        .on_press(|s: &mut Settings| s.confirm = true)
        .build();
    if s.confirm {
        save = save.overlay(
            Dialog::new(
                "Save settings?",
                "Apply these preferences to this device.",
                |s: &mut Settings| s.confirm = false,
            )
            .confirm("Save", |s: &mut Settings| {
                s.saved = true;
                s.confirm = false;
            })
            .build(),
        );
    }
    row()
        .h_full()
        .pad(token::PAD)
        .gap(token::GAP)
        .fill(token::CARD)
        .child(sidebar)
        .child(
            col()
                .grow(1.)
                .gap(token::GAP)
                .child(text("Device settings"))
                .child(body)
                .child(save),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Device settings",
        size: (720., 480.),
        kind: Kind::App { min: (520, 360) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: Settings::default(),
        view,
    })
}

```

## Try the flow

Select Downloads to see the list. Return to General, type a name, toggle Sync,
then choose Save changes. Cancel returns focus; Save closes the dialog and
changes the button label. The same flow can run on a virtual clock:

```json
[{"at":0.1,"click":"name"},{"at":0.2,"type":"Islam"},{"at":0.3,"click":"sync"},{"at":0.4,"click":"save"}]
```

Save that array as `settings-input.json` and run:

```sh
cargo run -p stillkit-playground --example ten_minutes -- --script settings-input.json --snap confirm.png
```

Use [the playground](playground.md) to compare controls across looks and states;
use [fast iteration](fast-iteration.md) for live look edits and Rust restarts.
