use std::time::Duration;
use stillkit::el::{El, Runtime, col, row, text, token};
use stillkit::{Image, Scheme, TextRole, Theme};
use stillkit_kit::*;
#[derive(Default)]
pub struct State {
    pub selected: usize,
    pub open: bool,
    pub value: f32,
    pub phase: u64,
    pub hidden: bool,
}
pub fn themes() -> Vec<(String, Theme)> {
    let mut out = Vec::new();
    for (scheme, suffix) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        let accent = stillkit::theme::SYSTEM_BLUE;
        for (name, t) in [
            ("liquid", Theme::liquid(scheme, accent)),
            ("solid", Theme::solid(scheme, accent)),
            ("oneui", Theme::one_ui(scheme, accent)),
            ("fluent", Theme::fluent(scheme, accent)),
            ("mono", Theme::mono(scheme, accent)),
            (
                "fluent-mono",
                Theme::fluent(scheme, accent)
                    .palette(stillkit::theme::Palette::mono(scheme, accent)),
            ),
        ] {
            out.push((format!("{name}-{suffix}"), t));
        }
    }
    out
}
fn card(title: &str, content: El<State>) -> El<State> {
    Card::new()
        .child(
            text(title.to_owned())
                .font(TextRole::Headline)
                .color(token::LABEL),
        )
        .child(content)
        .build()
}
pub fn view(s: &State) -> El<State> {
    let first = col()
        .w(330.)
        .gap(12.)
        .child(card(
            "Tabs",
            Tabs::new("Workspace", s.selected)
                .tab("Overview", text("All systems operational"))
                .tab("Activity", text("Recent changes appear here"))
                .tab("Settings", text("Workspace preferences"))
                .on_change(|s: &mut State, i| s.selected = i)
                .build()
                .key("tabs"),
        ))
        .child(card(
            "Disclosure",
            Disclosure::new(
                "Storage details",
                s.open,
                col()
                    .gap(8.)
                    .child(text("Documents · 24 GB"))
                    .child(text("Photos · 18 GB")),
            )
            .on_toggle(|s: &mut State| s.open = !s.open)
            .build()
            .key("disclosure"),
        ))
        .child(card(
            "Stepper",
            Stepper::new("Copies", 0., 10., 1., f64::from((s.value * 10.).round()))
                .on_change(|s: &mut State, v| s.value = v as f32 / 10.)
                .build()
                .key("stepper"),
        ))
        .child(card(
            "Ranges and ring",
            col()
                .gap(12.)
                .child(
                    Slider::new("Volume", s.value)
                        .on_change(|s: &mut State, v| s.value = v)
                        .build()
                        .key("slider"),
                )
                .child(Ring::new("Storage used", s.value).build().key("ring")),
        ))
        .child(card(
            "Loading and identity",
            row()
                .gap(16.)
                .child(
                    Spinner::new("Syncing", s.phase)
                        .visible(!s.hidden)
                        .on_tick(|s: &mut State| s.phase += 1)
                        .build(),
                )
                .child(Avatar::new("Islam", "IS").status("Available").build())
                .child(Avatar::new("Work", "WK").size(28.).build()),
        ))
        .child(card(
            "Skeleton",
            col()
                .gap(8.)
                .child(Skeleton::new(260., 16.).build())
                .child(Skeleton::new(200., 16.).build())
                .child(Skeleton::new(230., 48.).build()),
        ));
    let middle = col()
        .w(330.)
        .gap(12.)
        .child(card(
            "Table · fixed header",
            Table::new("Projects", ["Name", "Status"])
                .height(144.)
                .sorted(0, SortDirection::Ascending)
                .row("a", ["Stillkit", "Active"], s.selected == 0)
                .row("b", ["Studio", "Review"], s.selected == 1)
                .row("c", ["Archive", "Paused"], false)
                .row("d", ["Photos", "Ready"], false)
                .row("e", ["Notes", "Ready"], false)
                .on_sort(|s: &mut State, _| s.selected = 1)
                .on_select(|s: &mut State, k| s.selected = usize::from(k.as_ref() == "b"))
                .build()
                .key("table"),
        ))
        .child(card(
            "Tree",
            Tree::new(
                "Files",
                vec![
                    TreeNode::new("project", "Project")
                        .expanded(s.open)
                        .child(TreeNode::new("source", "Source").selected(true))
                        .child(TreeNode::new("assets", "Assets")),
                    TreeNode::new("archive", "Archive"),
                ],
            )
            .on_expand(|s: &mut State, (_, v)| s.open = v)
            .on_select(|s: &mut State, _| s.selected = 1)
            .build()
            .key("tree"),
        ))
        .child(card(
            "Scroll view",
            ScrollView::new("Recent files", 125.)
                .child(col().gap(8.).children(
                    (1..=12).map(|i| text(format!("Document {i:02}")).color(token::LABEL)),
                ))
                .build(),
        ))
        .child(card(
            "Radio and segments",
            col()
                .gap(12.)
                .child(
                    RadioGroup::new("Quality", ["Balanced", "Maximum"], s.selected)
                        .on_change(|s: &mut State, i| s.selected = i)
                        .build(),
                )
                .child(
                    Segmented::new("Period", ["Day", "Week", "Month"], s.selected)
                        .on_change(|s: &mut State, i| s.selected = i)
                        .build(),
                ),
        ));
    let last = col()
        .w(330.)
        .gap(12.)
        .child(card(
            "Sidebar",
            Sidebar::new("Workspace navigation")
                .section("Workspace")
                .item(
                    "overview",
                    "Overview",
                    Icon::Home,
                    s.selected == 0,
                    |s: &mut State| s.selected = 0,
                )
                .item(
                    "files",
                    "Files",
                    Icon::Folder,
                    s.selected == 1,
                    |s: &mut State| s.selected = 1,
                )
                .group(
                    Disclosure::new(
                        "Collections",
                        s.open,
                        col().child(text("Design system")).child(text("References")),
                    )
                    .on_toggle(|s: &mut State| s.open = !s.open),
                )
                .build(),
        ))
        .child(card(
            "Toolbar",
            Toolbar::new("Actions")
                .action("add", "Add", Icon::Folder, |_: &mut State| {})
                .action("settings", "Settings", Icon::Settings, |_: &mut State| {})
                .separator()
                .action("done", "Done", Icon::Check, |_: &mut State| {})
                .build(),
        ))
        .child(card(
            "Area chart",
            Chart::new(
                "Weekly usage",
                vec![
                    ChartSeries::new(
                        "Requests",
                        vec![2., 6., 4., 9., 7., 12., 10.],
                        token::ACCENT,
                    ),
                    ChartSeries::new("Cached", vec![1., 2., 3., 4., 3., 6., 5.], token::SECONDARY),
                ],
            )
            .x_labels(["Mon", "Thu", "Sun"])
            .build(),
        ))
        .child(card(
            "Quota",
            QuotaBar::new("Weekly allowance", 0.57)
                .pace(0.3)
                .badge("IS")
                .reset("3d 19h")
                .build(),
        ));
    col()
        .pad(18.)
        .gap(14.)
        .child(
            text("Layout and data")
                .font(TextRole::Title1)
                .color(token::LABEL),
        )
        .child(
            text("Public el primitives · themed geometry, ink and motion").color(token::SECONDARY),
        )
        .child(row().gap(14.).child(first).child(middle).child(last))
}
pub fn sheet(theme: Theme) -> anyhow::Result<Image> {
    sheet_at_scale(theme, 1)
}
/// Render at an integer device scale for the visual audit.
pub fn sheet_at_scale(theme: Theme, scale: u32) -> anyhow::Result<Image> {
    let mut r = Runtime::new(
        State {
            value: 0.57,
            open: true,
            ..Default::default()
        },
        view,
        theme.clone(),
        (1054 * scale, 1120 * scale),
        scale as f32,
    )?;
    r.set_background(Some(theme.color.surface.with_alpha(1.)));
    r.frame(Duration::ZERO);
    r.snap()
}
