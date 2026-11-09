//! F12 inspection and explicit native look-file reloads (`stillkit/inspect`).
use super::super::{Axis, col, el, text};
use super::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

/// A transient interaction state for a keyed story control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewState {
    /// Clear hover, press and keyboard focus.
    Rest,
    /// Pointer hover without invoking a handler.
    Hover,
    /// Held primary press without invoking a handler.
    Pressed,
    /// Visible keyboard focus.
    Focus,
}
impl<S> El<S> {
    /// Applies story validation and disables the complete subtree for a state preview.
    pub fn preview_flags(mut self, disabled: bool, invalid: bool) -> Self {
        fn disable<S>(e: &mut El<S>) {
            e.sem.disabled = true;
            for child in &mut e.children {
                disable(child);
            }
        }
        if disabled {
            disable(&mut self);
            self.look.opacity *= 0.45;
        }
        self.sem.invalid = invalid;
        if invalid {
            self = self.border(1., super::super::Paint::Theme(|t| t.color.destructive));
        }
        self
    }
}

/// A native subscription. Invalid edits leave the last valid theme in use.
pub struct ThemeWatch {
    _watch: crate::FileWatch,
    pending: Arc<Mutex<Option<Result<Theme>>>>,
}
impl ThemeWatch {
    /// Loads a look and watches byte changes, including atomic replacement.
    pub fn new(path: &Path, wake: crate::Waker) -> Result<(Self, Theme)> {
        let path = std::fs::canonicalize(path)?;
        let pending: Arc<Mutex<Option<Result<Theme>>>> = Arc::new(Mutex::new(None));
        let slot = pending.clone();
        let source = path.clone();
        #[cfg(windows)]
        let watch = crate::FileWatch::new(&path, move || {
            *slot.lock().unwrap() = Some(read_theme(&source));
            wake.wake();
        })?;
        #[cfg(windows)]
        {
            // Read after subscribing so a save during setup cannot be missed.
            let initial = read_theme(&path)?;
            Ok((
                Self {
                    _watch: watch,
                    pending,
                },
                initial,
            ))
        }
        #[cfg(not(windows))]
        {
            let _ = (slot, source, wake);
            anyhow::bail!("native look watching is currently available on Windows")
        }
    }
    /// Takes the newest edit, or its parse error.
    pub fn take(&self) -> Option<Result<Theme>> {
        self.pending.lock().unwrap().take()
    }
}
fn read_theme(path: &Path) -> Result<Theme> {
    parse_theme(path, &std::fs::read_to_string(path)?)
}
/// Parses the existing look format, rejecting unknown properties and invalid values.
pub fn parse_theme(path: &Path, source: &str) -> Result<Theme> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("json") => Theme::from_json(source),
        Some("toml") => Theme::from_toml(source),
        _ => anyhow::bail!("look files must end in .toml or .json"),
    }
}
#[derive(Default)]
pub(super) struct Inspector {
    panel: Option<Box<Runtime<Panel>>>,
    captured: bool,
}
#[derive(Default)]
struct Panel {
    rows: Vec<(String, String)>,
    details: Vec<String>,
    selected: Option<String>,
    heading: String,
}
fn panel_view(s: &Panel) -> El<Panel> {
    let mut tree = col().gap(2.).scroll(Axis::Vertical).grow(1.);
    for (i, (path, line)) in s.rows.iter().enumerate() {
        let path = path.clone();
        tree = tree.child(
            text(line.clone())
                .key(format!("inspect-{i}"))
                .on_press(move |s: &mut Panel| s.selected = Some(path.clone())),
        );
    }
    let mut body = col()
        .w(350.)
        .h_full()
        .pad(12.)
        .gap(5.)
        .fill(Rgba::new(0.06, 0.07, 0.09, 1.))
        .color(Rgba::new(0.94, 0.95, 0.97, 1.))
        .cascade_ink()
        .child(text(s.heading.clone()))
        .child(text("Follow pointer").on_press(|s: &mut Panel| s.selected = None));
    for line in &s.details {
        body = body.child(text(line.clone()).wrap(3));
    }
    el().h_full().child(body.absolute(0., 0.).child(tree))
}
impl Inspector {
    pub(super) fn input(&mut self, event: &Event) -> bool {
        let Some(panel) = &mut self.panel else {
            return false;
        };
        let left = panel.size.0 as f32 - 350. * panel.scale;
        let shift = |mut p: crate::Pointer| {
            p.at.x -= left.max(0.);
            p
        };
        let event = match *event {
            Event::Move(p) if self.captured || p.at.x >= left => Event::Move(shift(p)),
            Event::Press(p, b) if p.at.x >= left => {
                self.captured = true;
                Event::Press(shift(p), b)
            }
            Event::Release(p, b) if self.captured || p.at.x >= left => {
                self.captured = false;
                Event::Release(shift(p), b)
            }
            Event::Wheel(p, d) if p.at.x >= left => Event::Wheel(shift(p), d),
            _ => return false,
        };
        panel.handle(&event);
        true
    }
}
impl<S> Runtime<S> {
    /// Toggles inspection. Closing it releases all overlay allocations.
    pub fn set_inspector(&mut self, enabled: bool) {
        self.inspector = enabled.then(Inspector::default);
        self.layout.inspected = enabled.then(Vec::new);
        self.stale_paint = true;
    }
    /// Whether inspection is collecting or drawing anything.
    pub fn inspecting(&self) -> bool {
        self.inspector.is_some()
    }
    /// Starts a native subscription only when explicitly requested.
    pub fn watch_theme(&mut self, path: &Path) -> Result<()> {
        let (watch, theme) = ThemeWatch::new(path, crate::waker())?;
        self.set_theme(theme);
        self.look_watch = Some(watch);
        Ok(())
    }
    pub(super) fn tools_frame(&mut self, dt: Duration) -> bool {
        if let Some(next) = self.look_watch.as_ref().and_then(ThemeWatch::take) {
            match next {
                Ok(theme) => self.set_theme(theme),
                Err(e) => log::warn!("look reload: {e:#}"),
            }
        }
        let Some(mut inspector) = self.inspector.take() else {
            return self.frame_inner(dt);
        };
        let views = self.views;
        if let Some(inspected) = &mut self.layout.inspected {
            inspected.clear();
        }
        let started = Instant::now();
        self.stale_paint = true;
        self.frame_inner(dt);
        let elapsed = started.elapsed();
        let nodes = self.tree();
        let overlay = inspector.panel.get_or_insert_with(|| {
            Box::new(
                Runtime::new(
                    Panel::default(),
                    panel_view,
                    self.theme.clone(),
                    self.size,
                    self.scale,
                )
                .expect("inspector runtime"),
            )
        });
        let selected = overlay
            .state
            .selected
            .as_ref()
            .and_then(|path| nodes.iter().position(|n| &n.path == path))
            .or_else(|| {
                self.pointer
                    .and_then(|p| nodes.iter().rposition(|n| n.rect.contains(p.at)))
            });
        let ran = self.views != views;
        let mut details = Vec::new();
        let mut outline = el();
        if let Some(i) = selected.filter(|i| *i < nodes.len()) {
            let n = &nodes[i];
            details.push(format!(
                "Key: {}",
                n.key.as_deref().unwrap_or("(positional)")
            ));
            details.push(format!(
                "{:.0} x {:.0} at {:.0}, {:.0}",
                n.rect.width() / self.scale,
                n.rect.height() / self.scale,
                n.rect.left / self.scale,
                n.rect.top / self.scale
            ));
            each(&self.root, &self.laid, &mut |e, at, _| {
                if at == i {
                    let pad = e
                        .sizing
                        .pad
                        .map(|v| v.map(|l| l.resolve(&self.theme)).unwrap_or(0.));
                    let dim = |d: Option<super::super::element::Dim>| match d {
                        None => "auto".into(),
                        Some(super::super::element::Dim::Full) => "full".into(),
                        Some(super::super::element::Dim::Len(l)) => {
                            format!("{}", l.resolve(&self.theme))
                        }
                    };
                    details.push(format!(
                        "sizing {} x {} grow {}",
                        dim(e.sizing.w),
                        dim(e.sizing.h),
                        e.sizing.grow
                    ));
                    details.push(format!("padding TRBL {pad:?}"));
                    details.push(format!(
                        "radius {} ink {:?}",
                        self.targets.get(i).map_or(0., |t| t.radius / self.scale),
                        self.targets.get(i).map(|t| t.ink)
                    ));
                }
            });
            details.push(format!("fill {:?} opacity {}", n.fill, n.opacity));
            let r = n.rect;
            outline = el()
                .absolute(r.left / self.scale, r.top / self.scale)
                .w(r.width() / self.scale)
                .h(r.height() / self.scale)
                .border(1., Rgba::new(0., 0.8, 1., 1.));
        }
        let rows = nodes
            .iter()
            .enumerate()
            .map(|(index, n)| {
                let laid_out =
                    self.layout.inspected.as_ref().is_some_and(|ids| {
                        self.laid.get(index).is_some_and(|l| ids.contains(&l.id))
                    });
                (
                    n.path.clone(),
                    format!(
                        "{}{} {}",
                        if ran { "V" } else { " " },
                        if laid_out { "L" } else { " " },
                        n.path
                    ),
                )
            })
            .collect();
        overlay.resize(self.size);
        overlay.set_scale(self.scale);
        overlay.update(|s| {
            s.rows = rows;
            s.details = details;
            s.heading = format!(
                "F12 / {:.2} ms / {} touched\nV view rerun, L layout invalidated",
                elapsed.as_secs_f64() * 1000.,
                self.touched()
            );
        });
        overlay.frame_inner(dt);
        // A narrow panel placed at the right; its own tree can scroll and select nodes.
        let left = (self.size.0 as f32 - 350. * self.scale).max(0.);
        self.list.ops.extend(overlay.list.translated(left, 0.).ops);
        let mut highlight = Runtime::new((), |_| el(), self.theme.clone(), self.size, self.scale)
            .expect("inspector outline");
        highlight.root = el().child(outline);
        highlight.stale_view = false;
        highlight.stale_layout = true;
        highlight.frame_inner(Duration::ZERO);
        self.list.ops.extend(highlight.list.ops);
        self.window_list = None;
        self.inspector = Some(inspector);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    struct CountAlloc;
    thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
    fn allocated() {
        let _ = COUNT.try_with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
    }
    // SAFETY: every operation forwards the exact layout and pointer to System.
    unsafe impl GlobalAlloc for CountAlloc {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            allocated();
            // SAFETY: forwards the caller's allocation contract unchanged.
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            // SAFETY: forwards the caller's allocation contract unchanged.
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            allocated();
            // SAFETY: forwards the caller's allocation contract unchanged.
            unsafe { System.realloc(p, l, n) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            allocated();
            // SAFETY: forwards the caller's allocation contract unchanged.
            unsafe { System.alloc_zeroed(l) }
        }
    }
    #[global_allocator]
    static ALLOC: CountAlloc = CountAlloc;
    #[test]
    fn off_keeps_the_normal_frame_and_releases_inspection() {
        let mut rt = Runtime::new(
            (),
            |_| text("test"),
            Theme::solid(crate::Scheme::Dark, crate::theme::SYSTEM_BLUE),
            (400, 300),
            1.,
        )
        .unwrap();
        rt.frame(Duration::ZERO);
        assert!(!rt.inspecting());
        COUNT.with(|c| c.set(Some(0)));
        for _ in 0..100 {
            rt.frame(Duration::ZERO);
        }
        let allocations = COUNT.with(|c| c.replace(None).unwrap());
        assert_eq!(allocations, 0, "settled tools-off frames allocate nothing");
        let list = rt.list().clone();
        assert!(!rt.frame(Duration::ZERO));
        rt.set_inspector(true);
        rt.frame(Duration::ZERO);
        assert!(rt.list().ops.len() > list.ops.len());
        rt.set_inspector(false);
        rt.frame(Duration::ZERO);
        assert_eq!(rt.list(), &list);
        assert!(!rt.frame(Duration::ZERO));
    }
    #[test]
    fn f12_toggles_and_pointer_inspection_does_not_run_the_app_view() {
        let mut rt = Runtime::new(
            (),
            |_| text("target").key("target").size(80.),
            Theme::solid(crate::Scheme::Dark, crate::theme::SYSTEM_BLUE),
            (600, 400),
            1.,
        )
        .unwrap();
        rt.frame(Duration::ZERO);
        let runs = rt.view_runs();
        rt.key(0x7B);
        rt.pointer_move(20., 20.);
        rt.frame(Duration::ZERO);
        assert!(rt.inspecting());
        assert_eq!(rt.view_runs(), runs);
        rt.pointer_move(30., 30.);
        rt.frame(Duration::ZERO);
        assert_eq!(rt.view_runs(), runs);
        rt.key(0x7B);
        rt.frame(Duration::ZERO);
        assert!(!rt.inspecting());
    }
    #[test]
    fn panel_keeps_pointer_capture_until_release_outside() {
        let mut rt = Runtime::new(
            0,
            |_| text("app").size(100.).on_press(|n: &mut i32| *n += 1),
            Theme::solid(crate::Scheme::Dark, crate::theme::SYSTEM_BLUE),
            (800, 600),
            1.,
        )
        .unwrap();
        rt.set_inspector(true);
        rt.frame(Duration::ZERO);
        rt.pointer_down(500., 50.);
        assert!(rt.inspector.as_ref().unwrap().captured);
        rt.pointer_move(20., 20.);
        rt.pointer_up(20., 20.);
        assert!(!rt.inspector.as_ref().unwrap().captured);
        rt.frame(Duration::ZERO);
        assert_eq!(*rt.state(), 0);
    }
    #[test]
    fn forced_hover_previews_the_subject_without_invoking_it() {
        let mut rt = Runtime::new(
            0,
            |_| {
                col()
                    .child(
                        text("Subject")
                            .key("subject")
                            .size(80.)
                            .on_press(|n: &mut i32| *n += 1),
                    )
                    .child(text("Preview").key("preview").size(80.).on_press_with(
                        |_: &mut i32, tx| tx.preview_state("subject", PreviewState::Hover),
                    ))
            },
            Theme::solid(crate::Scheme::Dark, crate::theme::SYSTEM_BLUE),
            (600, 400),
            1.,
        )
        .unwrap();
        rt.set_reduced_motion(true);
        rt.frame(Duration::ZERO);
        assert!(rt.click("preview"));
        rt.frame(Duration::ZERO);
        assert_eq!(rt.presented("subject").unwrap().state, State::Hover);
        assert_eq!(*rt.state(), 0);
    }
    #[cfg(windows)]
    #[test]
    fn saved_looks_restyle_without_restarting_or_rerunning_the_view() -> Result<()> {
        let root =
            std::env::temp_dir().join(format!("stillkit-theme-watch-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        for (extension, first, second) in [
            ("toml", "base = 'mono'", "base = 'solid'"),
            ("json", r#"{"base":"mono"}"#, r#"{"base":"solid"}"#),
        ] {
            let path = root.join(format!("look.{extension}"));
            std::fs::write(&path, first)?;
            let mut rt = Runtime::new(
                42,
                |n| text(n.to_string()),
                Theme::solid(crate::Scheme::Dark, crate::theme::SYSTEM_BLUE),
                (400, 300),
                1.,
            )?;
            rt.watch_theme(&path)?;
            rt.frame(Duration::ZERO);
            let runs = rt.view_runs();
            let wait = |rt: &mut Runtime<i32>| {
                let deadline = Instant::now() + Duration::from_secs(5);
                while rt
                    .look_watch
                    .as_ref()
                    .unwrap()
                    .pending
                    .lock()
                    .unwrap()
                    .is_none()
                {
                    assert!(
                        Instant::now() < deadline,
                        "save did not wake the look watcher"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                rt.frame(Duration::ZERO);
            };
            std::fs::write(&path, "{")?;
            wait(&mut rt);
            assert_eq!(rt.theme, parse_theme(&path, first)?);
            let replacement = root.join("replacement");
            std::fs::write(&replacement, second)?;
            std::fs::rename(&replacement, &path)?;
            wait(&mut rt);
            assert_eq!(rt.theme, parse_theme(&path, second)?);
            assert_eq!(*rt.state(), 42);
            assert_eq!(rt.view_runs(), runs);
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn reload_parses_both_formats_and_rejects_partial_edits() {
        let a = parse_theme(Path::new("look.toml"), "base = 'mono'").unwrap();
        let b = parse_theme(Path::new("look.json"), r#"{"base":"mono"}"#).unwrap();
        assert_eq!(a, b);
        assert!(parse_theme(Path::new("look.json"), "{").is_err());
        assert!(parse_theme(Path::new("look.toml"), "unknown = 2").is_err());
    }
}
