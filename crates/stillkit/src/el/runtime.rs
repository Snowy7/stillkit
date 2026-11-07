//! The runtime: holds the state and the retained tree, feeds input to [`Interaction`], runs
//! handlers between frames and paints the tree into a [`DisplayList`].
//!
//! - **State changes** (a handler, [`Runtime::update`]) re-run the view, re-sync the retained
//!   layout by key and repaint.
//! - **Hover, press and focus** only repaint the retained tree with the new interaction state:
//!   the view does not run. Every pressable box keeps its wash and focus ring in the list
//!   (transparent at rest), so a hover changes one op and `Surface::present` redraws one
//!   control.
//! - **Size, scale and theme** re-lay out the retained tree without running the view.
//!
//! The headless methods (`pointer_down`, `click`, `find`, `presented`, `tree`, `snap`) drive
//! the same runtime the window host does, on a virtual clock.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use accesskit::Role;
use anyhow::Result;

use super::element::{El, Key, What};
use super::layout::{Cx, Laid, Layout};
use super::token;
use crate::interact::{Response, State};
use crate::{
    Button, DisplayList, Event, Frame, HitId, Image, Interaction, Op, Point, Pointer, Rect, Rgba,
    Surface, Theme, Ui, render,
};

/// Input waiting for its handler, run at the start of the next frame.
enum Action {
    Press(HitId),
    Drag(HitId, f32, f32),
}

/// A keyed element's layout and look, before or after a view run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub rect: Rect,
    pub fill: Option<Rgba>,
    pub opacity: f32,
}

/// One keyed element that came, went, moved or changed its look in the last re-layout.
///
/// This is where phase 4's animation tracks retarget: a track per key springs from `from` to
/// `to` (presence when either is `None`) instead of jumping. Today the runtime only records
/// them ([`Runtime::changes`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub key: Key,
    pub from: Option<Snapshot>,
    pub to: Option<Snapshot>,
}

/// What an element shows now: the `--tree` dump and [`Runtime::presented`].
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub key: Option<Key>,
    /// Keys (or places among unkeyed siblings) from the root, `/`-separated.
    pub path: String,
    /// Its hit item's role, when it has one.
    pub role: Option<Role>,
    pub label: Option<Arc<str>>,
    /// Device pixels.
    pub rect: Rect,
    pub fill: Option<Rgba>,
    pub text: Option<Arc<str>>,
    pub checked: Option<bool>,
    /// Hover and press, from the interaction state.
    pub state: State,
    pub focused: bool,
}

/// A view, its state and everything retained between frames. Headless as is; [`super::run`]
/// hosts one in a window.
pub struct Runtime<S> {
    state: S,
    view: Box<dyn Fn(&S) -> El<S>>,
    theme: Theme,
    ui: Ui,
    scale: f32,
    size: (u32, u32),
    background: Option<Rgba>,
    root: El<S>,
    laid: Vec<Laid>,
    layout: Layout,
    ix: Interaction,
    queue: Vec<Action>,
    dragging: Option<HitId>,
    stale_view: bool,
    stale_layout: bool,
    stale_paint: bool,
    list: DisplayList,
    damage: Option<Rect>,
    clock: Duration,
    views: u64,
    paints: u64,
    changes: Vec<Change>,
}

impl<S: 'static> Runtime<S> {
    /// A runtime `size` device pixels at `scale` device px per design px, with its first frame
    /// built.
    pub fn new(
        state: S,
        view: impl Fn(&S) -> El<S> + 'static,
        theme: Theme,
        size: (u32, u32),
        scale: f32,
    ) -> Result<Self> {
        let mut runtime = Self {
            state,
            view: Box::new(view),
            theme,
            ui: Ui::new()?,
            scale,
            size,
            background: None,
            root: super::el(),
            laid: Vec::new(),
            layout: Layout::new(),
            ix: Interaction::default(),
            queue: Vec::new(),
            dragging: None,
            stale_view: true,
            stale_layout: true,
            stale_paint: true,
            list: DisplayList::default(),
            damage: None,
            clock: Duration::ZERO,
            views: 0,
            paints: 0,
            changes: Vec::new(),
        };
        runtime.frame(Duration::ZERO);
        Ok(runtime)
    }
}

impl<S> Runtime<S> {
    /// Advances the virtual clock by `dt`, runs queued handlers, re-runs the view if the state
    /// changed, and repaints if anything it draws changed. Returns whether a new list was
    /// painted ([`damage`](Self::damage) says what it changed).
    pub fn frame(&mut self, dt: Duration) -> bool {
        self.clock += dt;
        if !self.queue.is_empty() {
            self.run_handlers();
        }
        if self.stale_view {
            self.root = (self.view)(&self.state);
            self.views += 1;
            self.stale_view = false;
            self.stale_layout = true;
        }
        if self.stale_layout {
            self.relayout();
        }
        if !self.stale_paint {
            return false;
        }
        self.paint();
        true
    }

    fn run_handlers(&mut self) {
        for action in std::mem::take(&mut self.queue) {
            let hit = match action {
                Action::Press(hit) | Action::Drag(hit, ..) => hit,
            };
            let Some(el) = element(&self.root, &self.laid, hit) else {
                continue;
            };
            match (action, &el.on_press, &el.on_drag) {
                (Action::Press(_), Some(handler), _) => handler(&mut self.state),
                (Action::Drag(_, x, y), _, Some(handler)) => handler(&mut self.state, x, y),
                _ => continue,
            }
            self.stale_view = true;
        }
    }

    fn relayout(&mut self) {
        let before = keyed(&self.root, &self.laid, &self.theme);
        let cx = Cx {
            theme: &self.theme,
            s: self.scale,
        };
        let size = (self.size.0 as f32, self.size.1 as f32);
        self.laid = self.layout.update(&self.root, size, &cx, &mut self.ui.text);
        let after = keyed(&self.root, &self.laid, &self.theme);
        self.retarget(before, after);
        self.stale_layout = false;
        self.stale_paint = true;
    }

    /// Diffs keyed elements across a re-layout. Phase 4 starts or retargets a track per
    /// [`Change`] here, and paints from the tracks instead of the targets.
    fn retarget(&mut self, before: HashMap<u64, (Key, Snapshot)>, mut after: HashMap<u64, (Key, Snapshot)>) {
        self.changes.clear();
        for (id, (key, from)) in before {
            match after.remove(&id) {
                Some((_, to)) if to == from => {}
                to => self.changes.push(Change {
                    key,
                    from: Some(from),
                    to: to.map(|(_, to)| to),
                }),
            }
        }
        self.changes.extend(after.into_values().map(|(key, to)| Change {
            key,
            from: None,
            to: Some(to),
        }));
    }

    fn paint(&mut self) {
        let mut f = Frame::new(&mut self.ui.text, &self.theme, self.scale, &self.ix);
        if let Some(background) = self.background {
            let (w, h) = self.size;
            f.fill(Rect::new(0.0, 0.0, w as f32, h as f32), 0.0, background);
        }
        paint_el(&mut f, &self.root, &self.laid, &mut 0);
        let mut list = f.list;
        list.focus = self.ix.focus;
        self.damage = list.damage(&self.list);
        self.list = list;
        self.paints += 1;
        self.stale_paint = false;
    }

    /// Feeds one input event. Hover, press and focus changes repaint at the next frame
    /// without running the view; activations queue their handlers.
    pub fn handle(&mut self, event: &Event) {
        if let Event::Key(0x09) = event {
            return self.tab(false);
        }
        let before = self.ix.clone();
        if let Response::Activate(hit) = self.ix.handle(&self.list, event) {
            self.queue.push(Action::Press(hit));
        }
        match *event {
            Event::Press(p, Button::Left) => {
                self.dragging = self
                    .ix
                    .pressed
                    .filter(|&hit| element(&self.root, &self.laid, hit).is_some_and(|el| el.on_drag.is_some()));
                self.drag(p.at);
            }
            Event::Move(p) => self.drag(p.at),
            Event::Release(..)
            | Event::Leave
            | Event::CaptureLost
            | Event::Cancelled
            | Event::Deactivated => self.dragging = None,
            _ => {}
        }
        if self.ix != before {
            self.stale_paint = true;
        }
    }

    fn drag(&mut self, at: Point) {
        let Some(hit) = self.dragging else { return };
        let Some(rect) = self.laid.iter().find(|l| l.hit == hit).map(|l| l.rect) else {
            return;
        };
        let along = |v: f32, start: f32, len: f32| ((v - start) / len.max(1.0)).clamp(0.0, 1.0);
        let (x, y) = (
            along(at.x, rect.left, rect.width()),
            along(at.y, rect.top, rect.height()),
        );
        // Only the latest place matters: one handler run per frame however fast it moves.
        if let Some(Action::Drag(last, ..)) = self.queue.last()
            && *last == hit
        {
            self.queue.pop();
        }
        self.queue.push(Action::Drag(hit, x, y));
    }

    /// Moves keyboard focus to the next (or, `back`, previous) focusable element (Tab).
    pub fn tab(&mut self, back: bool) {
        let before = self.ix.clone();
        self.ix.step(&self.list, if back { -1 } else { 1 });
        if self.ix != before {
            self.stale_paint = true;
        }
    }

    /// Changes the state outside a handler (a timer, IPC); the view runs at the next frame.
    pub fn update(&mut self, change: impl FnOnce(&mut S)) {
        change(&mut self.state);
        self.stale_view = true;
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    /// Resizes the surface (device px): re-lays out without running the view.
    pub fn resize(&mut self, size: (u32, u32)) {
        if size != self.size {
            self.size = size;
            self.stale_layout = true;
        }
    }

    /// Device px per design px (a DPI change): re-lays out without running the view.
    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.scale {
            self.scale = scale;
            self.stale_layout = true;
        }
    }

    /// A new look: every token resolves again; the view does not run.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.stale_layout = true;
    }

    /// An opaque fill under everything (app windows, which have no transparency).
    pub fn set_background(&mut self, background: Option<Rgba>) {
        if background != self.background {
            self.background = background;
            self.stale_paint = true;
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Device pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The thread's factories and text cache the runtime draws with.
    pub fn ui(&mut self) -> &mut Ui {
        &mut self.ui
    }

    /// The latest painted frame.
    pub fn list(&self) -> &DisplayList {
        &self.list
    }

    /// What the latest painted frame changed from the one before (`None`: nothing drawn).
    pub fn damage(&self) -> Option<Rect> {
        self.damage
    }

    /// The virtual clock: the sum of every `frame(dt)`.
    pub fn now(&self) -> Duration {
        self.clock
    }

    /// How many times the view ran.
    pub fn view_runs(&self) -> u64 {
        self.views
    }

    /// How many lists were painted.
    pub fn paints(&self) -> u64 {
        self.paints
    }

    /// Layout nodes created, restyled, re-measured or re-parented by the last re-layout.
    pub fn touched(&self) -> usize {
        self.layout.touched
    }

    /// Keyed elements changed by the last re-layout (see [`Change`]).
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    // ---- Headless input and queries (device px) --------------------------------------------

    pub fn pointer_move(&mut self, x: f32, y: f32) {
        self.handle(&Event::Move(pointer(x, y)));
    }

    pub fn pointer_down(&mut self, x: f32, y: f32) {
        self.handle(&Event::Press(pointer(x, y), Button::Left));
    }

    pub fn pointer_up(&mut self, x: f32, y: f32) {
        self.handle(&Event::Release(pointer(x, y), Button::Left));
    }

    /// A key going down (a virtual-key code: Enter 0x0D, Space 0x20, arrows, Escape).
    pub fn key(&mut self, vk: u16) {
        self.handle(&Event::Key(vk));
    }

    /// Presses and releases the pointer at the centre of the element keyed `key`; `false` if
    /// there is none.
    pub fn click(&mut self, key: &str) -> bool {
        let Some(at) = self.find(key).map(|r| r.center()) else {
            return false;
        };
        self.pointer_down(at.x, at.y);
        self.pointer_up(at.x, at.y);
        true
    }

    /// The laid-out rect (device px) of the first element keyed `key`.
    pub fn find(&self, key: &str) -> Option<Rect> {
        let mut found = None;
        each(&self.root, &self.laid, &mut |el, laid, _| {
            if found.is_none() && el.key.as_deref() == Some(key) {
                found = Some(laid.rect);
            }
        });
        found
    }

    /// What the first element keyed `key` shows now.
    pub fn presented(&self, key: &str) -> Option<Shown> {
        self.tree().into_iter().find(|s| s.key.as_deref() == Some(key))
    }

    /// Every element as shown now, in paint order.
    pub fn tree(&self) -> Vec<Shown> {
        let mut out = Vec::with_capacity(self.laid.len());
        each(&self.root, &self.laid, &mut |el, laid, path| {
            let hit = el.hit();
            let enabled = el.interactive() && !el.sem.disabled;
            out.push(Shown {
                key: el.key.clone(),
                path: path.to_owned(),
                role: hit.then(|| role_of(el)),
                label: hit.then(|| label_of(el)),
                rect: laid.rect,
                fill: el.look.fill.map(|p| p.resolve(&self.theme)),
                text: match &el.what {
                    What::Text { text, .. } => Some(text.clone()),
                    _ => None,
                },
                checked: el.sem.checked,
                state: if enabled { self.ix.state(laid.hit, true) } else { State::Rest },
                focused: self.ix.focus == Some(laid.hit),
            });
        });
        out
    }

    /// Shows the latest frame on `surface` at `origin` (screen px; app windows ignore it).
    /// The surface draws only what differs from what it shows.
    pub fn present(&mut self, surface: &mut Surface, origin: (i32, i32)) -> Result<bool> {
        surface.present(&mut self.ui, origin, self.size, self.list.clone())
    }

    /// The latest frame drawn headless, through the same software target as a surface.
    pub fn snap(&mut self) -> Result<Image> {
        render(&mut self.ui, &self.list, self.size.0, self.size.1)
    }
}

fn pointer(x: f32, y: f32) -> Pointer {
    Pointer {
        at: Point::new(x, y),
        screen: (x as i32, y as i32),
    }
}

/// The element whose hit id is `hit`.
fn element<'a, S>(root: &'a El<S>, laid: &[Laid], hit: HitId) -> Option<&'a El<S>> {
    fn walk<'a, S>(el: &'a El<S>, laid: &[Laid], i: &mut usize, hit: HitId) -> Option<&'a El<S>> {
        let me = laid.get(*i)?;
        *i += 1;
        if me.hit == hit {
            return Some(el);
        }
        el.children.iter().find_map(|child| walk(child, laid, i, hit))
    }
    walk(root, laid, &mut 0, hit)
}

/// Visits every element in pre-order with its layout and path.
fn each<S>(root: &El<S>, laid: &[Laid], visit: &mut impl FnMut(&El<S>, &Laid, &str)) {
    fn walk<S>(
        el: &El<S>,
        laid: &[Laid],
        i: &mut usize,
        path: &mut String,
        visit: &mut impl FnMut(&El<S>, &Laid, &str),
    ) {
        let Some(me) = laid.get(*i) else { return };
        *i += 1;
        visit(el, me, path);
        for (n, child) in el.children.iter().enumerate() {
            let len = path.len();
            path.push('/');
            match &child.key {
                Some(key) => path.push_str(key),
                None => path.push_str(&n.to_string()),
            }
            walk(child, laid, i, path, visit);
            path.truncate(len);
        }
    }
    walk(root, laid, &mut 0, &mut String::new(), visit);
}

fn keyed<S>(root: &El<S>, laid: &[Laid], theme: &Theme) -> HashMap<u64, (Key, Snapshot)> {
    let mut out = HashMap::new();
    each(root, laid, &mut |el, laid, _| {
        if let Some(key) = &el.key {
            let snapshot = Snapshot {
                rect: laid.rect,
                fill: el.look.fill.map(|p| p.resolve(theme)),
                opacity: el.look.opacity,
            };
            out.insert(laid.id, (key.clone(), snapshot));
        }
    });
    out
}

fn role_of<S>(el: &El<S>) -> Role {
    el.sem.role.unwrap_or(if el.on_drag.is_some() {
        Role::Slider
    } else if el.interactive() && el.sem.checked.is_some() {
        Role::Switch
    } else if el.interactive() {
        Role::Button
    } else {
        Role::Label
    })
}

fn label_of<S>(el: &El<S>) -> Arc<str> {
    el.sem
        .label
        .clone()
        .or_else(|| el.first_text().cloned())
        .unwrap_or_else(|| "".into())
}

/// Paints `el` and its children (pre-order, matching `laid`).
fn paint_el<S>(f: &mut Frame, el: &El<S>, laid: &[Laid], i: &mut usize) {
    let Some(&me) = laid.get(*i) else { return };
    *i += 1;
    let (theme, s, rect) = (f.theme, f.s, me.rect);
    let start = f.list.ops.len();
    let hits = f.list.hits.len();
    let enabled = el.interactive() && !el.sem.disabled;
    let radius = el
        .look
        .radius
        .map_or(0.0, |r| (r.resolve(theme) * s).round())
        .min(rect.width().min(rect.height()) / 2.0)
        .max(0.0);
    let ink = el.look.color.unwrap_or(token::LABEL).resolve(theme);
    let paints = matches!(el.what, What::Paint(_));
    if el.hit() && !paints {
        let label = label_of(el);
        f.hit(me.hit, rect, role_of(el), &label, enabled, el.sem.checked);
    }
    if el.look.shadow {
        let e = theme.elevation;
        f.push(Op::Shadow {
            rect,
            radius,
            spread: (e.spread * s).round(),
            drop: (e.drop * s).round(),
            color: e.color,
        });
    }
    if let Some(fill) = el.look.fill {
        f.fill(rect, radius, fill.resolve(theme));
    }
    if el.interactive() && !paints {
        f.wash(me.hit, rect, radius, enabled);
    }
    match &el.what {
        What::Box(_) => {}
        What::Text { text, role, wrap: None } => {
            let style = theme.font.style(*role, s);
            let fitted = f.text.fit(text, &style, rect.width());
            let (w, h) = f.text.measure(&fitted, &style);
            let top = (rect.top + (rect.height() - h) / 2.0).round();
            f.push(Op::Text {
                rect: Rect::new(rect.left, top, w, h),
                text: fitted,
                style,
                color: ink,
            });
        }
        What::Text { text, role, wrap: Some(lines) } => {
            f.paragraph(rect.left, rect.top, rect.width(), text, *role, ink, *lines);
        }
        What::Icon(icon) => f.icon(rect, *icon, rect.width().min(rect.height()) / s, ink),
        What::Image(image) if radius > 0.0 => f.push(Op::RoundedImage {
            rect,
            image: image.clone(),
            opacity: 1.0,
            source: None,
            radius,
        }),
        What::Image(image) => f.push(Op::Image {
            rect,
            image: image.clone(),
            opacity: 1.0,
            source: None,
        }),
        What::Paint(painter) => {
            painter(f, rect, me.hit);
            if el.hit() && !f.list.hits[hits..].iter().any(|h| h.id == me.hit) {
                let label = label_of(el);
                f.hit(me.hit, rect, role_of(el), &label, enabled, el.sem.checked);
            }
        }
    }
    for child in &el.children {
        paint_el(f, child, laid, i);
    }
    if el.interactive() && !paints {
        f.focus_ring(me.hit, rect, radius);
    }
    if el.look.opacity < 1.0 {
        fade(&mut f.list.ops[start..], el.look.opacity);
    }
}

/// Multiplies the alpha of everything `ops` draw by `by`.
fn fade(ops: &mut [Op], by: f32) {
    for op in ops {
        match op {
            Op::Continuous { color, .. }
            | Op::Emphasis { color, .. }
            | Op::Arc { color, .. }
            | Op::Fill { color, .. }
            | Op::Stroke { color, .. }
            | Op::Line { color, .. }
            | Op::Text { color, .. }
            | Op::Icon { color, .. }
            | Op::Shadow { color, .. } => *color = color.with_alpha(color.a * by),
            Op::Image { opacity, .. } | Op::RoundedImage { opacity, .. } => *opacity *= by,
            Op::Clipped { ops, .. } | Op::RoundedClipped { ops, .. } => fade(Arc::make_mut(ops), by),
            Op::TextField { .. } | Op::Clip { .. } | Op::Unclip => {}
        }
    }
}
