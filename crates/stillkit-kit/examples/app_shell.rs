//! An editor shell. `--snap-dir PATH` exports all five looks in light and dark, headlessly.
use std::time::{Duration, Instant};
use stillkit::el::{self, El, Runtime, col, text, token};
use stillkit::{Event, Scheme, Theme};
use stillkit_kit::*;

pub struct Model {
    pub split: SplitState,
    pub bottom: SplitState,
    pub tabs: Vec<Tab>,
    pub selected: std::sync::Arc<str>,
    pub strip: TabStripState,
    pub recorder: ShortcutRecorderState,
    pub hover: HoverCardState,
    pub expanded: bool,
    pub status: String,
    pub action: Option<WindowAction>,
    pub inset: f32,
    next: usize,
}
impl Default for Model {
    fn default() -> Self {
        let mut source = Tab::new("source", "main.rs");
        source.icon = Some(Icon::Check);
        source.dirty = true;
        Self {
            split: SplitState::new(220.),
            bottom: SplitState::new(300.),
            tabs: vec![
                source,
                Tab::new("readme", "README.md"),
                Tab::new("config", "Cargo.toml"),
            ],
            selected: "source".into(),
            strip: TabStripState::default(),
            recorder: ShortcutRecorderState::default(),
            hover: HoverCardState::default(),
            expanded: true,
            status: "Ready".into(),
            action: None,
            inset: 0.,
            next: 0,
        }
    }
}
pub fn tab_event(s: &mut Model, event: TabEvent) {
    match event {
        TabEvent::Select(id) => s.selected = id,
        TabEvent::Reorder(id, to) => {
            if let Some(from) = s.tabs.iter().position(|t| t.id == id) {
                let t = s.tabs.remove(from);
                s.tabs.insert(to.min(s.tabs.len()), t);
            }
        }
        TabEvent::Close(id) => {
            if s.tabs.iter().any(|t| t.id == id && t.dirty) {
                s.status = "Save main.rs before closing".into();
                return;
            }
            s.tabs.retain(|t| t.id != id);
            if s.selected == id {
                s.selected = s.tabs.first().map_or_else(|| "".into(), |t| t.id.clone());
            }
        }
        TabEvent::TornOut(id) => s.status = format!("Move {id} to a window (host event)"),
        TabEvent::New => {
            s.next += 1;
            let id = format!("new-{}", s.next);
            s.selected = id.clone().into();
            s.tabs.push(Tab::new(id, "Untitled"));
        }
    }
}
pub fn view(s: &Model) -> El<Model> {
    let tabs = TabStrip::new(
        "documents",
        s.tabs.clone(),
        s.selected.clone(),
        &s.strip,
        |s: &mut Model| &mut s.strip,
        tab_event,
    )
    .build();
    let titlebar = WindowTitlebar::new("titlebar", |s: &mut Model, a| s.action = Some(a))
        .system_inset(s.inset)
        .leading(text("Stillkit").color(token::LABEL).pad_x(10.))
        .tabs(tabs)
        .build();
    let sidebar = SidebarSection::new(SidebarHeader::new(
        "Project",
        s.tabs.len(),
        s.expanded,
        |s: &mut Model| s.expanded = !s.expanded,
    ))
    .child(
        SidebarRow::new("Source", Icon::Check, true, |s: &mut Model| {
            s.selected = "source".into()
        })
        .count(3)
        .build(),
    )
    .child(
        SidebarRow::new("Documentation", Icon::Add, false, |s: &mut Model| {
            s.selected = "readme".into()
        })
        .build(),
    )
    .build();
    let editor = Page::new(
        "Editor",
        col()
            .gap(token::GAP)
            .child(text("fn main() {").color(token::LABEL))
            .child(text("    println!(\"Hello, stillkit!\");").color(token::ACCENT))
            .child(text("}").color(token::LABEL)),
    )
    .toolbar(
        Button::new("Save")
            .on_press(|s: &mut Model| {
                if let Some(t) = s.tabs.iter_mut().find(|t| t.id == s.selected) {
                    t.dirty = false;
                }
            })
            .build(),
    )
    .build();
    let console = col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(text("Output").color(token::SECONDARY))
        .child(RollingText::new("status", s.status.clone()).build())
        .child(
            ShortcutRecorder::new("shortcut", "Open palette", &s.recorder, |s: &mut Model| {
                &mut s.recorder
            })
            .build(),
        )
        .child(
            Button::new("About this project")
                .key("about")
                .on_press(|s: &mut Model| {
                    let open = !s.hover.open;
                    s.hover.dismiss();
                    s.hover.open = open;
                })
                .build()
                .overlay(
                    HoverCard::new(
                        col()
                            .key("about-card")
                            .gap(token::ITEM)
                            .child(text("Native editor shell").color(token::LABEL))
                            .child(
                                PopupList::new(
                                    "quick-open",
                                    "Project files",
                                    s.selected.clone(),
                                    |s: &mut Model, id| s.selected = id,
                                )
                                .item(PopupItem::new("source", "main.rs"))
                                .item(PopupItem::new("readme", "README.md"))
                                .build(),
                            )
                            .child(
                                Button::new("Mark reviewed")
                                    .on_press(|s: &mut Model| s.status = "Reviewed".into())
                                    .build(),
                            ),
                        s.hover.open,
                        |s: &mut Model| {
                            s.hover.dismiss();
                        },
                    )
                    .key("about-panel")
                    .overlay(),
                ),
        );
    let main = SplitView::new(
        "console",
        &s.bottom,
        |s: &mut Model| &mut s.bottom,
        editor,
        console,
    )
    .axis(SplitAxis::Vertical)
    .limits(120., 600.)
    .reset_size(300.)
    .build();
    col()
        .w_full()
        .h_full()
        .fill(token::CARD)
        .child(titlebar)
        .child(
            SplitView::new(
                "workspace",
                &s.split,
                |s: &mut Model| &mut s.split,
                sidebar,
                main,
            )
            .limits(140., 400.)
            .collapsible(true)
            .reset_size(220.)
            .build()
            .h(0.)
            .grow(1.),
        )
}
pub fn themes() -> Vec<(String, Theme)> {
    let mut out = Vec::new();
    for (scheme, suffix) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        let accent = stillkit::theme::SYSTEM_BLUE;
        for (name, theme) in [
            ("liquid", Theme::liquid(scheme, accent)),
            ("oneui", Theme::one_ui(scheme, accent)),
            ("solid", Theme::solid(scheme, accent)),
            ("fluent", Theme::fluent(scheme, accent)),
            ("mono", Theme::mono(scheme, accent)),
        ] {
            out.push((format!("{name}-{suffix}"), theme));
        }
    }
    out
}
fn main() -> anyhow::Result<()> {
    if let Some(dir) = std::env::args().skip_while(|s| s != "--snap-dir").nth(1) {
        std::fs::create_dir_all(&dir)?;
        for (name, theme) in themes() {
            let background = theme.color.surface.with_alpha(1.);
            let mut rt = Runtime::new(Model::default(), view, theme, (1000, 720), 1.)?;
            rt.set_background(Some(background));
            rt.frame(Duration::ZERO);
            stillkit::golden::write(
                &rt.snap()?,
                &std::path::Path::new(&dir).join(format!("shell-{name}.png")),
            )?;
            rt.update(|s| {
                s.hover.open = true;
                s.recorder.chord = Some(KeyChord {
                    key: el::KeyCode::of('K'),
                    modifiers: el::Modifiers::CTRL | el::Modifiers::SHIFT,
                });
            });
            rt.frame(Duration::ZERO);
            rt.frame(Duration::from_secs(2));
            stillkit::golden::write(
                &rt.snap()?,
                &std::path::Path::new(&dir).join(format!("shell-rich-{name}.png")),
            )?;
        }
        return Ok(());
    }
    use stillkit::platform::current::host::{modifiers, wait, wake_at};
    use stillkit::{Caption, Kind, Role, Surface, next_event};
    let mut surface = Surface::new(Kind::App { min: (700, 520) }, "App shell", Role::Window)?;
    surface.set_caption(Caption::Compact);
    let scale = surface.scale();
    surface.place((160, 160, (1000. * scale) as i32, (720. * scale) as i32));
    let model = Model {
        inset: surface.system_controls().map_or(0., |r| r.right / scale),
        ..Model::default()
    };
    let mut rt = Runtime::new(
        model,
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        surface.client_size(),
        scale,
    )?;
    let background = rt.theme().color.surface.with_alpha(1.);
    rt.set_background(Some(background));
    surface.set_background(background);
    rt.set_reduced_motion(el::reduced_motion());
    rt.frame(Duration::ZERO);
    WindowTitlebar::sync_chrome(&surface, &rt, "titlebar");
    rt.present(&mut surface, (0, 0))?;
    surface.show_active(false);
    let started = Instant::now();
    let mut frames = None;
    loop {
        let deadline = [rt.wake_at(), rt.state().hover.deadline()]
            .into_iter()
            .flatten()
            .min();
        wake_at(deadline.map(|d| started + d));
        wait();
        while let Some((id, event)) = next_event() {
            if id != surface.id() {
                continue;
            }
            if event == Event::Close {
                return Ok(());
            }
            let mut recorder = rt.state().recorder.clone();
            let consumed = recorder.handle(&event, modifiers());
            if consumed || event == Event::Deactivated {
                rt.update(|s| s.recorder = recorder);
            }
            if let Some(anchor) = rt.find("about") {
                let card = rt.find("about-panel");
                let scale = rt.scale();
                // Runtime bounds are device pixels; HoverCard accepts design-pixel bounds.
                let logical = |r: stillkit::Rect| {
                    stillkit::Rect::new(
                        r.left / scale,
                        r.top / scale,
                        r.width() / scale,
                        r.height() / scale,
                    )
                };
                let mut hover = rt.state().hover.clone();
                if hover.handle(
                    &event,
                    logical(anchor),
                    card.map(logical),
                    scale,
                    started.elapsed(),
                ) {
                    rt.update(|s| s.hover = hover.clone());
                } else if hover.deadline() != rt.state().hover.deadline() {
                    rt.update(|s| s.hover = hover);
                }
            }
            if consumed {
                continue;
            }
            if let Some(event) =
                TabStripState::shortcut(&event, modifiers(), &rt.state().tabs, &rt.state().selected)
            {
                rt.update(|s| tab_event(s, event));
                let target = format!("documents/tab/{}", rt.state().selected);
                rt.focus(&target);
                rt.scroll_into_view(&target);
                continue;
            }
            match event {
                Event::Resized(w, h) => rt.resize((w, h)),
                Event::DpiChanged(_) => {
                    rt.set_scale(surface.scale());
                    rt.update(|s| {
                        s.inset = surface
                            .system_controls()
                            .map_or(0., |r| r.right / surface.scale())
                    });
                }
                Event::Key(vk) => rt.key_with(vk, modifiers()),
                event => rt.handle(&event),
            }
        }
        let mut hover = rt.state().hover.clone();
        if hover.advance(started.elapsed()) {
            rt.update(|s| s.hover = hover);
        }
        let redraw = rt.frame(started.elapsed().saturating_sub(rt.now()));
        if let Some(action) = rt.touch(|s| s.action.take()) {
            match action {
                WindowAction::Minimize => surface.minimize(),
                WindowAction::Maximize => surface.toggle_maximize(),
                WindowAction::Close => return Ok(()),
            }
        }
        if redraw {
            WindowTitlebar::sync_chrome(&surface, &rt, "titlebar");
            surface.set_cursor(rt.cursor());
            rt.present(&mut surface, (0, 0))?;
        }
        if rt.animating() {
            if frames.is_none() {
                frames = Some(surface.animation_frames(Duration::from_millis(16)));
            }
        } else {
            frames = None;
        }
    }
}
