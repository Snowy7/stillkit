//! Independent Runtime cells show actual open overlays without modal occlusion of other cells.
use std::time::Duration;
use stillkit::el::{self, El, Runtime, col, text};
use stillkit::theme::Material;
use stillkit::{Image, Scheme, Style, Theme};
use stillkit_kit::*;
#[derive(Default)]
pub struct State {
    pub menu: bool,
    pub context: Option<(f32, f32)>,
    pub select: bool,
    pub value: usize,
    pub dialog: bool,
    pub alert: bool,
    pub popover: bool,
    pub toast: bool,
    pub actions: usize,
}
pub fn menu(s: &State) -> El<State> {
    Button::new("Options")
        .key("menu-trigger")
        .on_press(|s: &mut State| s.menu = !s.menu)
        .build()
        .overlay(
            items()
                .on_dismiss(|s| s.menu = false)
                .overlay()
                .open(s.menu),
        )
}
fn items() -> Menu<State> {
    Menu::new()
        .section("Desktop")
        .item(
            MenuItem::new("New folder")
                .key("new")
                .icon(Icon::Folder)
                .shortcut("Ctrl+N")
                .on_press(|s: &mut State| s.actions += 1),
        )
        .item(
            MenuItem::new("Show desktop icons")
                .key("icons")
                .checked(true)
                .on_press(|s: &mut State| s.actions += 1),
        )
        .separator()
        .item(
            MenuItem::new("Sort by name")
                .key("sort")
                .radio(true)
                .on_press(|s: &mut State| s.actions += 1),
        )
        .item(
            MenuItem::new("Personalize")
                .key("disabled")
                .disabled(true)
                .on_press(|s: &mut State| s.actions += 100),
        )
}
pub fn context(s: &State) -> El<State> {
    ContextMenu::attach(
        Button::new("Right click here")
            .key("context-trigger")
            .on_press(|_: &mut State| {})
            .build(),
        s.context,
        items().on_dismiss(|s| s.context = None),
        |s, x, y| s.context = Some((x, y)),
    )
}
pub fn select(s: &State) -> El<State> {
    Select::new(
        "Size",
        ["Small", "Medium", "Large"],
        s.value,
        s.select,
        |s: &mut State, open| s.select = open,
        |s, value| s.value = value,
    )
    .key("size")
    .build()
    .w(250.)
}
pub fn dialog(s: &State) -> El<State> {
    Button::new("Open dialog")
        .key("dialog-trigger")
        .on_press(|s: &mut State| s.dialog = true)
        .build()
        .overlay(
            Dialog::new(
                "Save changes?",
                "Save your changes before leaving this page.",
                |s: &mut State| s.dialog = false,
            )
            .confirm("Save", |s| s.actions += 1)
            .build()
            .open(s.dialog),
        )
}
pub fn alert(s: &State) -> El<State> {
    Button::new("Delete item")
        .key("alert-trigger")
        .on_press(|s: &mut State| s.alert = true)
        .build()
        .overlay(
            AlertDialog::new(
                "Delete this item?",
                "This action cannot be undone.",
                |s: &mut State| s.alert = false,
            )
            .key("alert")
            .confirm("Delete", |s| s.actions += 1)
            .build()
            .open(s.alert),
        )
}
pub fn popover(s: &State) -> El<State> {
    Button::new("Share")
        .key("popover-trigger")
        .on_press(|s: &mut State| s.popover = !s.popover)
        .build()
        .overlay(
            el::popover(
                Popover::new(text("Anyone with the link can view.").color(el::token::SECONDARY))
                    .title("Share link")
                    .actions(
                        Button::new("Copy link")
                            .on_press(|s: &mut State| s.actions += 1)
                            .build(),
                    )
                    .build()
                    .key("share-panel"),
            )
            .open(s.popover)
            .on_dismiss(|s| s.popover = false),
        )
}
pub fn toast(s: &State) -> El<State> {
    let mut stack = Toaster::new();
    if s.toast {
        stack = stack.child(
            Toast::new("saved", "Your changes were saved")
                .action("Undo", |s: &mut State| s.actions += 1)
                .timeout(Duration::from_secs(5), |s| s.toast = false),
        );
    }
    col().overlay(stack.build())
}
pub fn tooltip(_: &State) -> El<State> {
    col().pad(90.).child(Tooltip::attach(
        Button::new("Battery")
            .key("battery")
            .on_press(|_: &mut State| {})
            .build(),
        "Battery 82%",
    ))
}
pub fn themes() -> Vec<(String, Theme)> {
    let mut themes = vec![];
    for scheme in [Scheme::Light, Scheme::Dark] {
        let suffix = if scheme == Scheme::Light {
            "light"
        } else {
            "dark"
        };
        for (name, style) in [
            ("liquid", Style::Liquid),
            ("oneui", Style::OneUi),
            ("fluent", Style::Fluent),
            ("solid", Style::Solid),
            ("mono", Style::Mono),
        ] {
            themes.push((
                format!("{name}-{suffix}"),
                Theme::new(style, scheme, stillkit::theme::SYSTEM_BLUE),
            ));
        }
        themes.push((
            format!("oneui-liquid-{suffix}"),
            Theme::one_ui(scheme, stillkit::theme::SYSTEM_BLUE).material(Material::liquid()),
        ));
    }
    themes
}
type Cell = (&'static str, fn(&State) -> El<State>);
pub fn sheet(theme: Theme) -> anyhow::Result<Image> {
    let cells: [Cell; 8] = [
        ("Menu", menu),
        ("Context menu", context),
        ("Select", select),
        ("Popover", popover),
        ("Dialog", dialog),
        ("Alert dialog", alert),
        ("Toast", toast),
        ("Tooltip", tooltip),
    ];
    let mut sheet = Image {
        width: 1600,
        height: 680,
        pixels: vec![0; 1600 * 680 * 4],
    };
    for (i, (name, view)) in cells.into_iter().enumerate() {
        let state = State {
            menu: true,
            context: Some((30., 80.)),
            select: true,
            value: 1,
            dialog: true,
            alert: true,
            popover: true,
            toast: true,
            ..State::default()
        };
        let mut rt = Runtime::new(
            state,
            move |s| {
                col()
                    .pad(20.)
                    .gap(12.)
                    .child(text(name).font(stillkit::TextRole::Caption))
                    .child(view(s))
            },
            theme.clone(),
            (400, 340),
            1.,
        )?;
        rt.set_background(Some(theme.color.surface.with_alpha(1.)));
        if i == 7 {
            let p = rt.find("battery").unwrap().center();
            rt.pointer_move(p.x, p.y);
        }
        rt.frame(Duration::from_millis(600));
        rt.frame(Duration::from_secs(2));
        let image = rt.snap()?;
        for y in 0..340 {
            let to = (((i as u32 / 4 * 340 + y) * 1600 + i as u32 % 4 * 400) * 4) as usize;
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
        let path = dir.join(format!("overlays-{name}.png"));
        stillkit::golden::write(&sheet(theme)?, &path)?;
        println!("{}", path.display());
    }
    Ok(())
}
