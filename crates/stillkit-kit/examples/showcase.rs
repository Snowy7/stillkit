//! Settings app. --snap/--tree/--script are supplied by el::run.
//! STILLKIT_SHOWCASE_LOOK=solid|mix; STILLKIT_SHOWCASE_SCHEME=light|dark.
use std::time::Duration;
use stillkit::el::{self, App, El, Paint, Tx, col, row, text, text_input, token};
use stillkit::theme::Material;
use stillkit::{Kind, Scheme, TextRole, Theme};
use stillkit_kit::*;
struct State {
    name: String,
    sync: bool,
    volume: f32,
    quality: usize,
    select: bool,
    page: usize,
    tab: usize,
    selected: String,
    dialog: bool,
    toast: bool,
    dark: bool,
    mixed: bool,
}
fn theme(s: &State) -> Theme {
    let scheme = if s.dark { Scheme::Dark } else { Scheme::Light };
    let accent = stillkit::theme::SYSTEM_BLUE;
    if s.mixed {
        Theme::one_ui(scheme, accent).material(Material::liquid())
    } else {
        Theme::solid(scheme, accent)
    }
}
// An opaque backing lets the theme switch repaint the entire client area.
const BACKGROUND: Paint = Paint::Theme(|t| t.color.surface.with_alpha(1.));
fn view(s: &State) -> El<State> {
    let appearance = Card::new()
        .child(text("Make yourself at home").font(TextRole::Title2))
        .child(text("A small native workspace, composed with el + kit.").color(token::SECONDARY))
        .child(
            row()
                .gap(12.)
                .child(
                    Button::new(if s.mixed {
                        "Look: One UI + Liquid"
                    } else {
                        "Look: Solid"
                    })
                    .key("look")
                    .kind(ButtonKind::Tinted)
                    .build()
                    .on_press_with(|s: &mut State, tx: &mut Tx| {
                        s.mixed = !s.mixed;
                        tx.set_theme(theme(s));
                    }),
                )
                .child(
                    Button::new(if s.dark {
                        "Scheme: Dark"
                    } else {
                        "Scheme: Light"
                    })
                    .key("scheme")
                    .kind(ButtonKind::Gray)
                    .build()
                    .on_press_with(|s: &mut State, tx: &mut Tx| {
                        s.dark = !s.dark;
                        tx.set_theme(theme(s));
                    }),
                ),
        )
        .child(
            text("Choose your look, then make this workspace yours.")
                .font(TextRole::Caption)
                .color(token::SECONDARY),
        )
        .build();
    let form = Card::new()
        .child(text("Workspace preferences").font(TextRole::Headline))
        .child(text("Workspace name").font(TextRole::Footnote))
        .child(
            text_input("name")
                .label("Workspace name")
                .value(s.name.as_str())
                .placeholder("Name your workspace")
                .pad(12.)
                .fill(token::WELL)
                .radius(token::RADIUS_SMALL)
                .on_input(|s: &mut State, value| s.name = value.into()),
        )
        .child(text("Sync across devices").font(TextRole::Footnote))
        .child(
            Switch::new("Sync across devices", s.sync)
                .key("sync")
                .on_toggle(|s: &mut State| s.sync = !s.sync)
                .build(),
        )
        .child(
            text(format!("Notification volume: {:.0}%", s.volume * 100.)).font(TextRole::Footnote),
        )
        .child(
            Slider::new("Notification volume", s.volume)
                .key("volume")
                .icon(Icon::Volume(2))
                .step(0.05)
                .on_change(|s: &mut State, value| s.volume = value)
                .build(),
        )
        .child(text("Preview quality").font(TextRole::Footnote))
        .child(
            Select::new(
                "Preview quality",
                ["Balanced", "High", "Original"],
                s.quality,
                s.select,
                |s: &mut State, open| s.select = open,
                |s, value| s.quality = value,
            )
            .key("quality")
            .build(),
        )
        .child(
            Button::new("Save preferences")
                .icon(Icon::Check)
                .key("save")
                .on_press(|s: &mut State| s.dialog = true)
                .build(),
        )
        .build();
    let table = Table::new("Connected devices", ["Device", "Status"])
        .height(150.)
        .row(
            "desktop",
            ["Desktop", "This device"],
            s.selected == "desktop",
        )
        .row(
            "laptop",
            ["Laptop", "Synced just now"],
            s.selected == "laptop",
        )
        .row("tablet", ["Tablet", "Offline"], s.selected == "tablet")
        .on_select(|s: &mut State, key| s.selected = key.to_string())
        .build();
    let content = if s.page == 0 {
        Tabs::new("Preferences", s.tab)
            .tab("General", form)
            .tab(
                "Devices",
                Card::new()
                    .child(text("Your connected devices").font(TextRole::Headline))
                    .child(table)
                    .build(),
            )
            .on_change(|s: &mut State, tab| s.tab = tab)
            .build()
    } else {
        Card::new()
            .child(text("About this workspace").font(TextRole::Title2))
            .child(text(
                "Built with stillkit, stillkit-kit and the native element host.",
            ))
            .child(
                text("CPU rendering by default. No background sync is performed by this demo.")
                    .wrap(2)
                    .color(token::SECONDARY),
            )
            .build()
    };
    let mut toaster = Toaster::new();
    if s.toast {
        toaster = toaster.child(
            Toast::new("saved", "Preferences saved for this session")
                .timeout(Duration::from_secs(4), |s: &mut State| s.toast = false),
        );
    }
    row()
        .fill(BACKGROUND)
        .animate()
        .child(
            col()
                .w(205.)
                .h_full()
                .pad(20.)
                .gap(24.)
                .fill(token::WELL)
                .child(text("stillkit").font(TextRole::Title1))
                .child(
                    Sidebar::new("Settings navigation")
                        .section("WORKSPACE")
                        .item(
                            "preferences",
                            "Preferences",
                            Icon::Settings,
                            s.page == 0,
                            |s: &mut State| s.page = 0,
                        )
                        .item(
                            "about",
                            "About",
                            Icon::Home,
                            s.page == 1,
                            |s: &mut State| s.page = 1,
                        )
                        .build(),
                )
                .child(
                    text("A quieter kind of app.")
                        .font(TextRole::Caption)
                        .color(token::SECONDARY),
                ),
        )
        .child(
            col()
                .grow(1.)
                .pad(28.)
                .gap(20.)
                .scroll(el::Axis::Vertical)
                .child(text("Settings").font(TextRole::Title1))
                .child(appearance)
                .child(content),
        )
        .overlay(
            Dialog::new(
                "Save preferences?",
                "Changes are kept for this demo session.",
                |s: &mut State| s.dialog = false,
            )
            .confirm("Save", |s| s.toast = true)
            .build()
            .open(s.dialog),
        )
        .overlay(toaster.build())
}
fn main() -> anyhow::Result<()> {
    let state = State {
        name: "My workspace".into(),
        sync: true,
        volume: 0.6,
        quality: 1,
        select: false,
        page: 0,
        tab: 0,
        selected: "desktop".into(),
        dialog: false,
        toast: false,
        dark: std::env::var("STILLKIT_SHOWCASE_SCHEME").as_deref() != Ok("light"),
        mixed: std::env::var("STILLKIT_SHOWCASE_LOOK").as_deref() != Ok("solid"),
    };
    el::run(App {
        title: "stillkit settings",
        size: (980., 820.),
        kind: Kind::App { min: (820, 700) },
        theme: theme(&state),
        state,
        view,
    })
}
