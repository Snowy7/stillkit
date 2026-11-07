//! Headless text-component sheets, with independent cells for simultaneous overlay states.
use std::time::Duration;
use stillkit::el::{El, Runtime, col, text};
use stillkit::{Image, Theme};
use stillkit_kit::*;
#[path = "kit_overlays.rs"]
#[allow(dead_code)]
mod overlays;
pub use overlays::themes;
#[derive(Default)]
pub struct State {
    pub value: String,
    pub revealed: bool,
    pub combo: QueryState,
    pub palette: QueryState,
    pub ran: usize,
    pub selected: usize,
    pub select_open: bool,
}
pub fn field(s: &State) -> El<State> {
    TextField::new("Name")
        .key("field")
        .value(s.value.as_str())
        .placeholder("Your name")
        .on_input(|s: &mut State, t| s.value = t.into())
        .build()
}
pub fn search(s: &State) -> El<State> {
    SearchField::new("Search")
        .key("search")
        .value(s.value.as_str())
        .placeholder("Search library")
        .on_input(|s: &mut State, t| s.value = t.into())
        .build()
}
pub fn secure(s: &State) -> El<State> {
    SecureField::new("Password")
        .key("secure")
        .value(s.value.as_str())
        .revealed(s.revealed)
        .on_reveal(|s: &mut State, v| s.revealed = v)
        .on_input(|s, t| s.value = t.into())
        .build()
}
pub fn area(s: &State) -> El<State> {
    TextArea::new("Notes")
        .key("notes")
        .rows(3)
        .max_len(12)
        .character_count(true)
        .value(s.value.as_str())
        .on_input(|s: &mut State, t| s.value = t.into())
        .build()
}
pub fn combo(s: &State) -> El<State> {
    Combobox::new(
        "Workspace",
        ["Design", "Development", "Research"],
        &s.combo,
        |s: &mut State, q| s.combo = q,
    )
    .key("combo")
    .build()
}
pub fn palette(s: &State) -> El<State> {
    Button::new("Commands")
        .key("commands")
        .on_press(|s: &mut State| s.palette.open = true)
        .build()
        .overlay(
            CommandPalette::new(&s.palette, |s: &mut State, q| s.palette = q)
                .command(
                    Command::new("new", "New document", |s: &mut State| s.ran += 1)
                        .group("Create")
                        .icon(Icon::Folder)
                        .shortcut("Ctrl+N"),
                )
                .command(
                    Command::new("folder", "Open folder", |s: &mut State| s.ran += 10)
                        .group("Navigate")
                        .icon(Icon::Folder)
                        .shortcut("Ctrl+O"),
                )
                .command(
                    Command::new("settings", "Settings", |s: &mut State| s.ran += 100)
                        .group("Navigate")
                        .icon(Icon::Search),
                )
                .build(),
        )
}
pub fn select(s: &State) -> El<State> {
    Select::new(
        "Size",
        ["Small", "Medium", "Large"],
        s.selected,
        s.select_open,
        |s: &mut State, v| s.select_open = v,
        |s, v| s.selected = v,
    )
    .key("size")
    .build()
}
pub fn sheet(theme: Theme) -> anyhow::Result<Image> {
    let mut sheet = Image {
        width: 1600,
        height: 760,
        pixels: vec![0; 1600 * 760 * 4],
    };
    for i in 0..8 {
        let state = State {
            value: if matches!(i, 2 | 5) {
                "Stillkit".into()
            } else {
                String::new()
            },
            combo: QueryState {
                open: true,
                ..Default::default()
            },
            palette: QueryState {
                open: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let name = [
            "Rest",
            "Focused",
            "Filled search",
            "Error",
            "Disabled / read only",
            "Secure / text area",
            "Combobox",
            "Command palette",
        ][i];
        let mut rt = Runtime::new(
            state,
            move |s| {
                col().pad(20.).gap(12.).child(text(name)).child(match i {
                    0 | 1 => field(s),
                    2 => search(s),
                    3 => TextField::new("Email")
                        .key("error")
                        .value("not an email")
                        .error("Enter a valid email address")
                        .build(),
                    4 => col()
                        .gap(12.)
                        .child(
                            TextField::new("Disabled")
                                .disabled(true)
                                .value("Unavailable")
                                .build(),
                        )
                        .child(
                            TextField::new("Read only")
                                .key("readonly")
                                .value("Reference value")
                                .read_only(true)
                                .build(),
                        ),
                    5 => col().gap(16.).child(secure(s)).child(area(s)),
                    6 => combo(s),
                    _ => palette(s),
                })
            },
            theme.clone(),
            (400, 380),
            1.,
        )?;
        rt.set_background(Some(theme.color.surface.with_alpha(1.)));
        if i == 1 {
            rt.tab(false);
        }
        rt.frame(Duration::from_secs(1));
        rt.frame(Duration::from_secs(2));
        let image = rt.snap()?;
        for y in 0..380 {
            let to = (((i as u32 / 4 * 380 + y) * 1600 + i as u32 % 4 * 400) * 4) as usize;
            let from = (y * 400 * 4) as usize;
            sheet.pixels[to..to + 1600].copy_from_slice(&image.pixels[from..from + 1600]);
        }
    }
    Ok(sheet)
}
#[allow(dead_code)]
fn main() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/media/kit");
    for (name, theme) in themes() {
        let path = dir.join(format!("text-{name}.png"));
        stillkit::golden::write(&sheet(theme)?, &path)?;
        println!("{}", path.display());
    }
    Ok(())
}
