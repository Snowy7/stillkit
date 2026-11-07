//! Shared headless sheets used by the executable and golden tests.
use std::time::Duration;
use stillkit::el::{El, Runtime, col, icon, row, text, token};
use stillkit::theme::{Material, Palette};
use stillkit::{Image, Scheme, Theme};
use stillkit_kit::*;

#[derive(Default)]
pub struct State {
    pub on: bool,
    pub value: f32,
    pub selected: usize,
    pub day: u8,
    pub presses: usize,
}
pub const NAMES: [&str; 22] = [
    "Filled button",
    "Tinted button",
    "Gray button",
    "Plain button",
    "Destructive button",
    "Icon button",
    "Switch",
    "Checkbox",
    "Radio group",
    "Thin slider",
    "Volume bar",
    "Segments",
    "Progress",
    "Badge",
    "List row",
    "Section header",
    "Tile",
    "Card",
    "Separator",
    "Empty state",
    "Field frame",
    "Calendar",
];
pub const STATES: [&str; 6] = [
    "Rest",
    "Hover",
    "Pressed",
    "Focus visible",
    "Disabled",
    "Checked / full",
];
pub fn component(i: usize, s: &State, disabled: bool) -> El<State> {
    match i {
        0 => col().gap(5.).children(
            [
                ButtonKind::Filled,
                ButtonKind::Tinted,
                ButtonKind::Gray,
                ButtonKind::Plain,
                ButtonKind::Destructive,
            ]
            .into_iter()
            .enumerate()
            .map(|(j, k)| {
                Button::new(["Save", "Tinted", "Gray", "Plain", "Delete"][j])
                    .kind(k)
                    .key(if j == 0 {
                        "target".into()
                    } else {
                        format!("button-{j}")
                    })
                    .icon(if j == 4 { Icon::Delete } else { Icon::Check })
                    .disabled(disabled)
                    .on_press(|s: &mut State| s.presses += 1)
                    .build()
            }),
        ),
        1 => IconButton::new(Icon::Settings, "Settings")
            .key("target")
            .disabled(disabled)
            .on_press(|s: &mut State| s.presses += 1)
            .build(),
        2 => Switch::new("Wi-Fi", s.on)
            .key("target")
            .disabled(disabled)
            .on_toggle(|s: &mut State| s.on = !s.on)
            .build(),
        3 => Checkbox::new("Include subfolders", s.on)
            .key("target")
            .disabled(disabled)
            .on_toggle(|s: &mut State| s.on = !s.on)
            .build(),
        4 => RadioGroup::new("Quality", ["Balanced", "High", "Maximum"], s.selected)
            .key("target")
            .disabled(disabled)
            .on_change(|s: &mut State, i| s.selected = i)
            .build(),
        5 => Slider::new("Brightness", s.value)
            .key("target")
            .step(0.1)
            .disabled(disabled)
            .on_change(|s: &mut State, v| s.value = v)
            .build(),
        6 => Slider::new("Volume", s.value)
            .step(0.1)
            .key("target")
            .icon(Icon::Volume(2))
            .disabled(disabled)
            .on_change(|s: &mut State, v| s.value = v)
            .build(),
        7 => Segmented::new("Mode", ["Day", "Week", "Month"], s.selected)
            .key("target")
            .disabled(disabled)
            .on_change(|s: &mut State, i| s.selected = i)
            .build(),
        8 => Progress::new("Download", s.value).build(),
        9 => Badge::new("Ready").build(),
        10 => ListRow::new("Headphones")
            .subtitle("Connected")
            .lead(icon(Icon::Headphones).size(24.))
            .accessory(icon(Icon::ChevronRight).size(16.))
            .key("target")
            .disabled(disabled)
            .on_press(|s: &mut State| s.presses += 1)
            .build(),
        11 => SectionHeader::new("Connections").build(),
        12 => Tile::new("Bluetooth", Icon::Bluetooth, s.on)
            .subtitle(if s.on { "On" } else { "Off" })
            .key("target")
            .disabled(disabled)
            .on_toggle(|s: &mut State| s.on = !s.on)
            .build(),
        13 => Card::new()
            .child(text("A quieter surface").font(stillkit::TextRole::Headline))
            .child(
                text("Concentric corners and theme padding")
                    .wrap(2)
                    .color(token::SECONDARY),
            )
            .build(),
        14 => Separator.build(),
        15 => EmptyState::new(
            "Nothing here yet",
            "Add a folder to get started",
            Icon::Folder,
        )
        .build(),
        16 => {
            #[allow(deprecated)]
            let legacy = TextFieldFrame::new("Search")
                .placeholder("Search your library")
                .value(if s.on { "Nature" } else { "" })
                .disabled(disabled)
                .build()
                .key("target");
            legacy
        }
        17 => CalendarMonth::new(2026, 11)
            .today(7)
            .selected(s.day)
            .key("target")
            .disabled(disabled)
            .on_select(|s: &mut State, d| s.day = d)
            .build(),
        _ => unreachable!(),
    }
}
pub fn themes() -> Vec<(String, Theme)> {
    let mut themes = Vec::new();
    for (scheme, suffix) in [(Scheme::Dark, "dark"), (Scheme::Light, "light")] {
        let accent = stillkit::theme::SYSTEM_BLUE;
        for (name, t) in [
            ("liquid", Theme::liquid(scheme, accent)),
            ("oneui", Theme::one_ui(scheme, accent)),
            ("solid", Theme::solid(scheme, accent)),
            ("fluent", Theme::fluent(scheme, accent)),
            ("mono", Theme::mono(scheme, accent)),
            (
                "studio",
                Theme::solid(scheme, accent).palette(Palette::mono(scheme, accent)),
            ),
            (
                "oneui-liquid",
                Theme::one_ui(scheme, accent).material(Material::liquid()),
            ),
            (
                "fluent-mono",
                Theme::fluent(scheme, accent).palette(Palette::mono(scheme, accent)),
            ),
        ] {
            themes.push((format!("{name}-{suffix}"), t));
        }
    }
    themes
}
fn cell_height(i: usize) -> u32 {
    match i {
        8 => 150,
        17 => 160,
        19 => 170,
        21 => 360,
        _ => 88,
    }
}
fn gallery_component(i: usize, s: &State, disabled: bool) -> El<State> {
    if i < 5 {
        Button::new(["Save", "Share", "Cancel", "Learn more", "Delete"][i])
            .kind(
                [
                    ButtonKind::Filled,
                    ButtonKind::Tinted,
                    ButtonKind::Gray,
                    ButtonKind::Plain,
                    ButtonKind::Destructive,
                ][i],
            )
            .key("target")
            .icon(if i == 4 { Icon::Delete } else { Icon::Check })
            .disabled(disabled)
            .on_press(|s: &mut State| s.presses += 1)
            .build()
    } else {
        component(i - 4, s, disabled)
    }
}
pub fn sheet(theme: Theme) -> anyhow::Result<Image> {
    sheet_at_scale(theme, 1)
}
/// Render at an integer device scale for the visual audit.
pub fn sheet_at_scale(theme: Theme, scale: u32) -> anyhow::Result<Image> {
    let width = 6 * 280 * scale;
    let height = (40
        + NAMES
            .iter()
            .enumerate()
            .map(|(i, _)| cell_height(i))
            .sum::<u32>())
        * scale;
    let mut sheet = Image {
        width,
        height,
        pixels: vec![0; (width * height * 4) as usize],
    };
    for (state, name) in STATES.into_iter().enumerate() {
        let mut title = Runtime::new(
            (),
            move |_| {
                row()
                    .pad(10.)
                    .child(text(name).font(stillkit::TextRole::Headline))
            },
            theme.clone(),
            (280 * scale, 40 * scale),
            scale as f32,
        )?;
        title.set_background(Some(theme.color.surface.with_alpha(1.)));
        title.frame(Duration::ZERO);
        paste(&mut sheet, &title.snap()?, state as u32 * 280 * scale, 0);
        let mut y = 40 * scale;
        for (i, name) in NAMES.into_iter().enumerate() {
            let s = State {
                on: state == 5,
                value: if state == 5 { 1. } else { 0.45 },
                selected: if state == 5 { 2 } else { 0 },
                day: if state == 5 { 21 } else { 7 },
                presses: 0,
            };
            let mut rt = Runtime::new(
                s,
                move |s| {
                    col()
                        .pad(12.)
                        .gap(8.)
                        .child(
                            text(name)
                                .font(stillkit::TextRole::Caption)
                                .color(token::SECONDARY),
                        )
                        .child(gallery_component(i, s, state == 4))
                },
                theme.clone(),
                (280 * scale, cell_height(i) * scale),
                scale as f32,
            )?;
            rt.set_background(Some(theme.color.surface.with_alpha(1.)));
            rt.frame(Duration::ZERO);
            let target = if i == 8 || i == 11 {
                "target/option-0"
            } else if i == 21 {
                "target/day-7"
            } else {
                "target"
            };
            if let Some(r) = rt.find(target) {
                let p = r.center();
                match state {
                    1 => rt.pointer_move(p.x, p.y),
                    2 => rt.pointer_down(p.x, p.y),
                    3 => rt.tab(false),
                    _ => {}
                }
                rt.frame(Duration::ZERO);
            }
            rt.frame(Duration::from_secs(2));
            paste(&mut sheet, &rt.snap()?, state as u32 * 280 * scale, y);
            y += cell_height(i) * scale;
        }
    }
    Ok(sheet)
}
fn paste(dst: &mut Image, src: &Image, x: u32, y: u32) {
    for row in 0..src.height {
        let start = (((y + row) * dst.width + x) * 4) as usize;
        let from = (row * src.width * 4) as usize;
        dst.pixels[start..start + src.width as usize * 4]
            .copy_from_slice(&src.pixels[from..from + src.width as usize * 4]);
    }
}
