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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn edit_navigate_and_confirm_settings() -> anyhow::Result<()> {
        let mut rt = el::Runtime::new(
            Settings::default(),
            view,
            Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
            (720, 480),
            1.,
        )?;
        rt.set_reduced_motion(true);
        rt.frame(Duration::ZERO);
        assert!(rt.click("name"));
        rt.frame(Duration::ZERO);
        rt.type_text("Islam");
        rt.frame(Duration::ZERO);
        assert_eq!(rt.state().name, "Islam");
        assert!(rt.click("downloads"));
        rt.frame(Duration::ZERO);
        assert!(rt.find("notes").is_some());
        assert!(rt.click("general"));
        rt.frame(Duration::ZERO);
        assert!(rt.click("save"));
        rt.frame(Duration::ZERO);
        assert!(rt.state().confirm);
        assert!(rt.click("dialog/confirm"));
        rt.frame(Duration::ZERO);
        assert!(rt.state().saved);
        assert!(!rt.state().confirm);
        Ok(())
    }
}
