//! The runtime: holds the state and the retained tree, feeds input to [`Interaction`], runs
//! handlers between frames, animates toward what the view asks for and paints the tree into a
//! [`DisplayList`].
//!
//! - **State changes** (a handler, [`Runtime::update`]) re-run the view, re-sync the retained
//!   layout by key and retarget the animation tracks ([`super::animate`]).
//! - **Animation frames** only advance the tracks and repaint: the view does not run.
//! - **Hover, press and focus** only repaint the retained tree with the new interaction state:
//!   the view does not run. Every pressable box keeps its wash and focus ring in the list
//!   (transparent at rest), so a hover changes one op and `Surface::present` redraws one
//!   control.
//! - **Scrolling** moves a scroll view's children back by its offset at paint time: the view
//!   does not run, nothing is laid out again, and only the view's clipped group changes.
//! - **Keys** go to the focused element and up through its ancestors ([`El::on_key`], then
//!   each one's range, focus group or scroll view), then to the defaults. Tab and arrows move
//!   focus over the retained tree's focus stops (a focus group is one stop), so a stop hidden
//!   in a scroll view is reached and scrolled into view.
//! - **Size, scale and theme** re-lay out the retained tree without running the view (size
//!   and scale land at once; a theme's colours animate where elements have motion).
//! - **Text fields** ([`text`]) take keys and characters before their ancestors, keep their
//!   caret, selection, undo and scroll here, and hand edits to `on_input`; on Windows the
//!   focused one types through RichEdit (`native`).
//! - **Overlays** ([`layer`]) are lifted out of the tree after each view run and appended to
//!   the root, placed against their anchors, and take Escape, outside presses and focus first.
//!   Tooltips and timers run on the clock without running the view.
//!
//! The headless methods (`pointer_down`, `click`, `find`, `presented`, `tree`, `snap`) drive
//! the same runtime the window host does, on a virtual clock.

use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;
use std::time::Duration;

use accesskit::Role;
use anyhow::Result;

mod layer;
#[cfg(windows)]
mod native;
mod text;

use super::animate::{Animator, Mode, Pose, Target};
use super::element::{ArcLook, Axis, El, Handled, Key, KeyCode, Modifiers, Range, Transition, Tx, What};
use super::feedback::Feedback;
use super::field::Field;
use super::layout::{Cx, Laid, Layout};
use super::overlay::{Dismiss, Kind};
use layer::{Entry, Layer, MENU, Relation, Tip};
pub use layer::Popup;
use super::scroll::{LINE, NOTCH, ScrollView};
use super::token;
use crate::a11y::Detail;
use crate::interact::{Response, State};
use crate::{
    AccessibilityAction, Button, DisplayList, Event, Frame, Hit, HitId, Image, Interaction, Op, Point, Pointer,
    Rect, Rgba, Semantics, Surface, Theme, Ui, render,
};

/// A view: the element tree for a state.
type View<S> = Box<dyn Fn(&S) -> El<S>>;

/// Input waiting for its handler, run at the start of the next frame.
/// A press an `on_pointer` element holds until release (device px).
#[derive(Debug, Clone, Copy)]
struct Capture {
    hit: HitId,
    button: Button,
    origin: Point,
    last: Point,
}

enum Action<S> {
    Press(HitId),
    Drag(HitId, f32, f32),
    /// The pointer's story on an element with `on_pointer`.
    Pointer(HitId, super::element::PointerEvent),
    /// The wheel over an element with `on_wheel`.
    Wheel(HitId, super::element::WheelEvent),
    /// Files dropped on an element with `on_drop_files`.
    Files(HitId, Vec<std::path::PathBuf>),
    /// A range stepped by this much.
    Step(HitId, f64),
    /// A context menu asked for at this point (design px).
    Context(HitId, f32, f32),
    Timeout(HitId),
    /// An overlay should close.
    Dismiss(Dismiss<S>),
    /// A field's text after an edit (`on_input`).
    Input(HitId, String),
    /// Enter in a field (`on_submit`).
    Submit(HitId, String),
}

/// A keyed element's layout and look, before or after a view run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    /// Bounds in device pixels.
    pub rect: Rect,
    /// Fill.
    pub fill: Option<Rgba>,
    /// Opacity.
    pub opacity: f32,
}

/// One keyed element that came, went, moved or changed its look in the last re-layout, from
/// its layout target before to its target after (presence when either is `None`). The
/// animation tracks retarget from the same re-layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Key.
    pub key: Key,
    /// From.
    pub from: Option<Snapshot>,
    /// To.
    pub to: Option<Snapshot>,
}

/// What an element shows now: the `--tree` dump and [`Runtime::presented`].
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// Key.
    pub key: Option<Key>,
    /// Keys (or places among unkeyed siblings) from the root, `/`-separated.
    pub path: String,
    /// Its hit item's role, when it has one.
    pub role: Option<Role>,
    /// Text announced to assistive technology.
    pub label: Option<Arc<str>>,
    /// Device pixels, where it is drawn now (mid-animation, between its layouts; scrolled).
    pub rect: Rect,
    /// Fill.
    pub fill: Option<Rgba>,
    /// Text content.
    pub text: Option<Arc<str>>,
    /// Checked.
    pub checked: Option<bool>,
    /// Hover and press, from the interaction state.
    pub state: State,
    /// Focused.
    pub focused: bool,
    /// Its own opacity now, entrance included.
    pub opacity: f32,
    /// A range's min, max and value ([`El::range`]).
    pub range: Option<(f64, f64, f64)>,
    /// Its value as text ([`El::value`]).
    pub value: Option<Arc<str>>,
    /// A text field's selection: where it started and the caret (byte offsets in `text`).
    pub selection: Option<(usize, usize)>,
    /// Description.
    pub description: Option<Arc<str>>,
    /// Selected.
    pub selected: Option<bool>,
    /// Expanded.
    pub expanded: Option<bool>,
    /// A scroll view's offset now, device px along its axis.
    pub scroll: Option<f32>,
    /// Its value fails validation ([`El::invalid`]).
    pub invalid: bool,
    /// The key of the descendant it shows as active ([`El::active_descendant`]).
    pub active_descendant: Option<Key>,
}

/// An element that left the view and stays drawn, out of hit testing, until its exit settles.
struct Ghost<S> {
    id: u64,
    parent: u64,
    /// Its place among its parent's children when it left.
    index: usize,
    el: El<S>,
    /// Its subtree's layout when it left (hit ids and text widths).
    laid: Vec<Laid>,
    /// Its subtree as last shown, local rects relative to its parent's.
    frozen: Vec<Pose>,
}

/// A keyboard focus stop, in paint order: an actionable hit item.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stop {
    hit: HitId,
    /// The element that pushed it, in `laid`.
    index: usize,
    /// The focus group it belongs to (the group element's place in `laid`).
    group: Option<usize>,
    /// Selected or checked: where Tab enters its group.
    selected: bool,
}

/// A scroll view as paint draws it.
#[derive(Debug, Clone, Copy)]
struct Bar {
    axis: Axis,
    offset: f32,
    max: f32,
    alpha: f32,
}

/// A view, its state and everything retained between frames. Headless as is; [`super::run`]
/// hosts one in a window.
pub struct Runtime<S> {
    state: S,
    view: View<S>,
    theme: Theme,
    ui: Ui,
    scale: f32,
    size: (u32, u32),
    background: Option<Rgba>,
    root: El<S>,
    laid: Vec<Laid>,
    targets: Vec<Target>,
    /// What each laid-out element showed in the last paint.
    poses: Vec<Pose>,
    ghosts: Vec<Ghost<S>>,
    layout: Layout,
    anim: Animator,
    time_scale: f32,
    /// How the next re-layout lands.
    mode: Mode,
    ix: Interaction,
    queue: Vec<Action<S>>,
    dragging: Option<HitId>,
    /// Scroll views by identity.
    scrolls: HashMap<u64, ScrollView>,
    /// Whether a scroll view glides or its scrollbar shows (frames keep coming).
    scrolling: bool,
    /// Keys to scroll into view once the layout is current.
    reveal: Vec<Key>,
    /// A [`El::follow_end`] view jumped to its end in the last layout.
    followed: bool,
    /// Range steps queued since the view last ran, by hit, so quick keys add up.
    stepped: HashMap<HitId, f64>,
    /// Focus stops of the last paint.
    nav: Vec<Stop>,
    /// Accessibility values of the last paint, by hit.
    semantics: Vec<(HitId, Semantics)>,
    semantics_sent: bool,
    /// Where the pointer last was, so hover follows content scrolling under it.
    pointer: Option<Pointer>,
    stale_view: bool,
    stale_layout: bool,
    stale_paint: bool,
    list: DisplayList,
    damage: Option<Rect>,
    clock: Duration,
    views: u64,
    paints: u64,
    changes: Vec<Change>,
    // ---- Overlays (`layer`) ------------------------------------------------------------
    /// Open overlays in paint order; their entries are the root's last children.
    layers: Vec<Layer<S>>,
    /// Where the entries start among the root's children.
    entries: usize,
    /// Tooltip triggers of the last view run, by identity.
    tips: Vec<(u64, Arc<str>)>,
    tip: Tip,
    /// Elements with a timeout, by identity: when it fires (`None` once it has).
    timers: HashMap<u64, Option<Duration>>,
    /// Where focus goes back to when an overlay that took it closes, by its identity.
    returns: HashMap<u64, Option<HitId>>,
    /// The overlays open at the last relayout.
    shown: Vec<u64>,
    /// Anchor rects last seen, by overlay (an orphan stays where it was).
    anchors: HashMap<u64, Rect>,
    /// Overlays already told that their anchor went.
    orphans: HashSet<u64>,
    /// Triggers' and panels' accessibility relations, by identity.
    relations: HashMap<u64, Relation>,
    /// Where overlays may go beyond the window ([`Runtime::set_screen`]).
    screen: Option<Rect>,
    /// Overlays drawn outside the window at the last paint, and the window's list without them.
    popups: Vec<Popup>,
    window_list: Option<DisplayList>,
    /// Overlays drawn outside the window, open or leaving.
    lifted: HashMap<u64, Rect>,
    // ---- Text fields (`text`) ---------------------------------------------------------
    fields: HashMap<u64, Field>,
    /// The field with focus, by identity, as last seen.
    typing: Option<u64>,
    selecting: Option<text::Selecting>,
    clipboard: Box<dyn super::Clipboard>,
    /// The modifier keys held with the key being handled.
    mods: Modifiers,
    /// The modifiers held, as the host last said ([`Runtime::set_modifiers`]).
    held: Modifiers,
    /// The press an `on_pointer` element holds, and where the pointer last hovered one.
    capture: Option<Capture>,
    hovered_at: Option<(HitId, Point)>,
    /// The element dragged files are over that would take them (its place in `laid`).
    drop_target: Option<usize>,
    /// Whether the window has the keyboard (the caret blinks and shows only then).
    active: bool,
    /// Whether the last paint drew the caret.
    caret_shown: bool,
    /// Hover and press washes and focus rings easing in and out.
    feedback: Feedback,
    /// The latest list's hits as the pointer meets them, when some are gone through
    /// (announced content inside a control); `None`: the list's own.
    pointing: Option<DisplayList>,
    // ---- Focus control --------------------------------------------------------------------
    /// Where `Tx::focus`/`blur` (or `Runtime::focus`/`blur`) move focus once the view has run,
    /// and whether a key asked.
    focus_to: Option<(Option<Key>, bool)>,
    /// Elements with a state look ([`El::hovered`], [`El::pressed`], [`El::focus_within`]),
    /// by identity.
    stateful: Vec<u64>,
    /// Those of them in each state now (innermost first).
    states_on: super::element::Active,
    /// Elements that set a pointer shape (and text fields, an I-beam), by identity.
    cursors: Vec<(u64, crate::Cursor)>,
    // ---- Spinning (`El::spin`) -------------------------------------------------------
    /// Each spinner's turn when it last showed, by identity.
    spins: HashMap<u64, f32>,
    /// Whether the last paint had something visible spinning.
    spinning: bool,
    /// Whether the window shows ([`Runtime::set_visible`]).
    visible: bool,
}

impl<S: 'static> Runtime<S> {
    /// A runtime `size` device pixels at `scale` device px per design px, with its first frame
    /// built (nothing animates on it).
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
            targets: Vec::new(),
            poses: Vec::new(),
            ghosts: Vec::new(),
            layout: Layout::new(),
            anim: Animator::default(),
            time_scale: 1.0,
            mode: Mode::First,
            ix: Interaction::default(),
            queue: Vec::new(),
            dragging: None,
            scrolls: HashMap::new(),
            scrolling: false,
            reveal: Vec::new(),
            followed: false,
            stepped: HashMap::new(),
            nav: Vec::new(),
            semantics: Vec::new(),
            semantics_sent: false,
            pointer: None,
            stale_view: true,
            stale_layout: true,
            stale_paint: true,
            list: DisplayList::default(),
            damage: None,
            clock: Duration::ZERO,
            views: 0,
            paints: 0,
            changes: Vec::new(),
            layers: Vec::new(),
            entries: 0,
            tips: Vec::new(),
            tip: Tip::default(),
            timers: HashMap::new(),
            returns: HashMap::new(),
            shown: Vec::new(),
            anchors: HashMap::new(),
            orphans: HashSet::new(),
            relations: HashMap::new(),
            screen: None,
            popups: Vec::new(),
            window_list: None,
            lifted: HashMap::new(),
            fields: HashMap::new(),
            typing: None,
            selecting: None,
            clipboard: Box::new(super::clipboard::Memory::default()),
            mods: Modifiers::NONE,
            held: Modifiers::NONE,
            capture: None,
            hovered_at: None,
            drop_target: None,
            active: true,
            caret_shown: false,
            feedback: Feedback::default(),
            focus_to: None,
            pointing: None,
            stateful: Vec::new(),
            states_on: Default::default(),
            cursors: Vec::new(),
            spins: HashMap::new(),
            spinning: false,
            visible: true,
        };
        runtime.frame(Duration::ZERO);
        Ok(runtime)
    }
}

impl<S> Runtime<S> {
    /// Advances the virtual clock by `dt`, runs queued handlers, re-runs the view if the state
    /// changed, advances the animation, and repaints if anything it draws changed. Returns
    /// whether a new list was painted ([`damage`](Self::damage) says what it changed).
    pub fn frame(&mut self, dt: Duration) -> bool {
        self.clock += dt;
        self.anim.t += dt.as_secs_f64() * f64::from(self.time_scale);
        self.fire_timers();
        // A dismissal found while laying out (an anchor that went) runs in the same frame.
        for _ in 0..3 {
            if !self.queue.is_empty() {
                self.run_handlers();
            }
            if !(self.stale_view || self.stale_layout) {
                break;
            }
            let before = keyed(&self.root, &self.laid, &self.theme);
            let old = self.stale_view.then(|| {
                self.views += 1;
                self.stale_view = false;
                self.stepped.clear();
                let root = self.run_view();
                let old = std::mem::replace(&mut self.root, root);
                self.hoist();
                old
            });
            self.relayout(before, old);
        }
        if let Some((to, keyed)) = self.focus_to.take() {
            self.move_focus(to, keyed);
        }
        self.follow_states();
        if !self.reveal.is_empty() {
            for key in std::mem::take(&mut self.reveal) {
                if let Some(index) = self.index_of_key(&key) {
                    self.reveal_index(index);
                }
            }
        }
        let moved = self.anim.moving();
        let left = self.anim.settle();
        if !left.is_empty() {
            self.ghosts.retain(|g| !left.contains(&g.id));
        }
        let scrolled = self.tick_scrolls() || std::mem::take(&mut self.followed);
        // A virtual list scrolled past what it built: build again, in this frame.
        if scrolled && self.virtual_stale() {
            let before = keyed(&self.root, &self.laid, &self.theme);
            self.views += 1;
            let root = self.run_view();
            let old = std::mem::replace(&mut self.root, root);
            self.hoist();
            self.relayout(before, Some(old));
        }
        // Overlays follow an anchor that scrolled.
        if scrolled && self.layers.iter().any(|l| l.anchor != super::overlay::Anchor::Window) {
            let before = keyed(&self.root, &self.laid, &self.theme);
            self.relayout(before, None);
        }
        self.sync_focus();
        // Feedback that started since the last frame shows one display frame in already, so
        // the frame after a hover, press or Tab is never a still one.
        let lead = f64::from(self.time_scale) / 60.0;
        let glowing = self.sync_feedback(self.anim.t - lead);
        if moved || glowing || self.spinning || !left.is_empty() || scrolled || self.caret_on() != self.caret_shown {
            self.stale_paint = true;
        }
        if !self.stale_paint {
            return false;
        }
        self.paint();
        // Content scrolled under a still pointer: hover what is there now.
        if scrolled && let Some(p) = self.pointer {
            let hover = self.ix.hover;
            self.ix.handle(self.pointing.as_ref().unwrap_or(&self.list), &Event::Move(p));
            if self.ix.hover != hover {
                self.follow_states();
                self.paint();
            }
        }
        true
    }

    /// The pointer's shape now: that of the innermost element under the pointer (or holding
    /// a press that began on it) that sets one ([`El::cursor`]; text fields show an I-beam),
    /// else the arrow. Hosts apply it after input ([`run`](super::run) does).
    pub fn cursor(&self) -> crate::Cursor {
        let mut at = self.ix.pressed.or(self.ix.hover).and_then(|hit| self.index_of_hit(hit));
        while let Some(i) = at.filter(|&i| i < self.laid.len()) {
            let id = self.laid[i].id;
            if let Some(&(_, cursor)) = self.cursors.iter().find(|(c, _)| *c == id) {
                return cursor;
            }
            at = self.laid[i].parent;
        }
        crate::Cursor::Arrow
    }

    /// Restyles the elements with a state look ([`El::hovered`], [`El::pressed`],
    /// [`El::focus_within`]) when the pointer or focus moved in or out of them, or a press
    /// began or ended: their targets change (springing along their motion); the view does
    /// not run.
    fn follow_states(&mut self) {
        if self.stateful.is_empty() && self.states_on == Default::default() {
            return;
        }
        let chain = |hit: Option<crate::HitId>| {
            let mut on = Vec::new();
            let mut at = hit.and_then(|hit| self.index_of_hit(hit));
            while let Some(i) = at.filter(|&i| i < self.laid.len()) {
                let id = self.laid[i].id;
                if self.stateful.contains(&id) {
                    on.push(id);
                }
                at = self.laid[i].parent;
            }
            on
        };
        // Pressed shows only while the pointer is still over what was pressed, as washes do.
        let pressed = self.ix.pressed.filter(|&p| self.ix.hover == Some(p));
        let drop = self.drop_target.and_then(|i| self.laid.get(i)).map(|l| vec![l.id]).unwrap_or_default();
        let active = super::element::Active { hover: chain(self.ix.hover), within: chain(self.ix.focus), press: chain(pressed), drop };
        if active == self.states_on {
            return;
        }
        self.states_on = active;
        let targets = targets(&self.root, &self.laid, &self.theme, self.scale, &self.states_on);
        let all: HashSet<u64> = self.laid.iter().map(|l| l.id).collect();
        self.anim.retarget(&self.laid, &targets, &all, Mode::Animate);
        self.targets = targets;
        self.stale_paint = true;
    }

    /// Heads washes and focus rings for the interaction state (from `start` on the clock) and
    /// lands what settled; whether any of them shows something new.
    fn sync_feedback(&mut self, start: f64) -> bool {
        let field = self.typing.map(super::layout::hit_of);
        let ring = self.ix.focus.filter(|&f| self.ix.focus_visible || field == Some(f));
        let spring = (!self.anim.reduced).then_some(self.theme.motion.interactive);
        let moving = self.feedback.moving();
        let changed = self.feedback.sync(&self.ix, ring, spring, start);
        self.feedback.settle(self.anim.t);
        changed || moving
    }

    /// Settles the scroll views; whether what any of them shows changed since the last paint.
    fn tick_scrolls(&mut self) -> bool {
        if self.scrolls.is_empty() {
            self.scrolling = false;
            return false;
        }
        let (t, theme) = (self.anim.t, &self.theme);
        let mut changed = false;
        self.scrolling = false;
        for view in self.scrolls.values_mut() {
            self.scrolling |= view.settle(t, theme);
            changed |= (view.offset(t), view.bar(t, theme)) != view.shown;
        }
        changed
    }

    fn run_handlers(&mut self) {
        let mut tx = Tx::default();
        for action in std::mem::take(&mut self.queue) {
            let hit = match &action {
                Action::Dismiss(handler) => {
                    handler(&mut self.state);
                    self.stale_view = true;
                    continue;
                }
                Action::Press(hit)
                | Action::Input(hit, _)
                | Action::Submit(hit, _)
                | Action::Drag(hit, ..)
                | Action::Pointer(hit, _)
                | Action::Wheel(hit, _)
                | Action::Files(hit, _)
                | Action::Step(hit, _)
                | Action::Context(hit, ..)
                | Action::Timeout(hit) => *hit,
            };
            let Some(el) = element(&self.root, &self.laid, hit) else {
                continue;
            };
            let state = &mut self.state;
            match action {
                Action::Press(_) => match &el.on_press {
                    Some(handler) => handler(state, &mut tx),
                    None => continue,
                },
                Action::Drag(_, x, y) => match &el.on_drag {
                    Some(handler) => {
                        handler(state, x, y);
                        tx.instant();
                    }
                    None => continue,
                },
                Action::Pointer(_, event) => match &el.on_pointer {
                    Some(handler) => {
                        if !handler(state, &event, &mut tx) {
                            continue;
                        }
                        tx.instant();
                    }
                    None => continue,
                },
                Action::Files(_, paths) => match &el.on_files {
                    Some(handler) => handler(state, &paths),
                    None => continue,
                },
                Action::Wheel(_, event) => match &el.on_wheel {
                    Some(handler) if handler(state, &event, &mut tx) => {}
                    _ => continue,
                },
                Action::Step(_, delta) => match &el.on_step {
                    Some(handler) => handler(state, delta),
                    None => continue,
                },
                Action::Context(_, x, y) => match &el.on_context {
                    Some(handler) => handler(state, x, y),
                    None => continue,
                },
                Action::Timeout(_) => match &el.on_timeout {
                    Some((_, handler)) => handler(state),
                    None => continue,
                },
                Action::Input(_, text) => match &el.on_input {
                    Some(handler) => handler(state, &text),
                    None => continue,
                },
                Action::Submit(_, text) => match &el.on_submit {
                    Some(handler) => handler(state, &text),
                    None => continue,
                },
                Action::Dismiss(_) => continue,
            }
            self.stale_view = true;
        }
        self.apply(tx, false);
    }

    /// What a handler asked of its frame; `keyed` when a key ran it (a focus it moves shows
    /// its ring).
    fn apply(&mut self, tx: Tx, keyed: bool) {
        if tx.instant {
            self.land_next();
        }
        if let Some(theme) = tx.theme {
            self.set_theme(theme);
        }
        self.reveal.extend(tx.reveal);
        if let Some(to) = tx.focus {
            self.focus_to = Some((to, keyed));
        }
    }

    /// Moves keyboard focus to the first focusable element keyed `key` (`None`: nowhere),
    /// showing its ring if `keyed` or the ring already showed, and scrolls it into view.
    fn move_focus(&mut self, to: Option<Key>, keyed: bool) {
        let Some(key) = to else {
            if self.ix.focus.is_some() {
                (self.ix.focus, self.ix.focus_visible) = (None, false);
                self.stale_paint = true;
            }
            return;
        };
        let mut found = None;
        each(&self.root, &self.laid, &mut |el, i, _| {
            if found.is_none() && el.key.as_deref() == Some(&*key) && el.interactive() && !el.sem.disabled {
                found = Some(i);
            }
        });
        let Some(index) = found else { return };
        let visible = keyed || self.ix.focus_visible;
        let hit = self.laid[index].hit;
        if (self.ix.focus, self.ix.focus_visible) != (Some(hit), visible) {
            (self.ix.focus, self.ix.focus_visible) = (Some(hit), visible);
            self.stale_paint = true;
        }
        self.reveal_index(index);
    }

    /// Moves keyboard focus to the first focusable element keyed `key` once the view has run
    /// (at the next frame), keeping whether its ring shows, and scrolls it into view.
    pub fn focus(&mut self, key: &str) {
        self.focus_to = Some((Some(key.into()), false));
    }

    /// Takes keyboard focus away at the next frame: nothing has it.
    pub fn blur(&mut self) {
        self.focus_to = Some((None, false));
    }

    /// The next re-layout lands with no motion (the first frame stays the first).
    fn land_next(&mut self) {
        if self.mode == Mode::Animate {
            self.mode = Mode::Instant;
        }
    }

    fn relayout(&mut self, before: HashMap<u64, (Key, Snapshot)>, old: Option<El<S>>) {
        let mode = std::mem::replace(&mut self.mode, Mode::Animate);
        let mut laid = self.lay_out();
        if !self.layers.is_empty() {
            laid = self.place_layers(laid);
        }
        self.stateful.clear();
        self.cursors.clear();
        each(&self.root, &laid, &mut |el, i, _| {
            if el.states.any() {
                self.stateful.push(laid[i].id);
            }
            let field = el.field().map(|_| crate::Cursor::Text);
            if let Some(cursor) = el.cursor.or(field) {
                self.cursors.push((laid[i].id, cursor));
            }
        });
        let targets = targets(&self.root, &laid, &self.theme, self.scale, &self.states_on);
        let ids: HashSet<u64> = laid.iter().map(|l| l.id).collect();
        if let Some(old) = old
            && mode == Mode::Animate
        {
            self.leave(old, &ids);
        }
        // Ghosts go when their parent does, or when they come back.
        self.ghosts.retain(|g| ids.contains(&g.parent) && !ids.contains(&g.id));
        let ghosts: HashSet<u64> = self.ghosts.iter().map(|g| g.id).collect();
        self.anim.retain(|id| ids.contains(&id) || ghosts.contains(&id));
        self.spins.retain(|id, _| ids.contains(id));
        let prev: HashSet<u64> = self.laid.iter().map(|l| l.id).collect();
        self.anim.retarget(&laid, &targets, &prev, mode);
        self.laid = laid;
        self.targets = targets;
        self.sync_fields();
        self.sync_scrolls();
        self.sync_timers();
        if !self.layers.is_empty() || !self.shown.is_empty() {
            self.follow_layers();
        }
        let after = keyed(&self.root, &self.laid, &self.theme);
        self.diff(before, after);
        self.stale_layout = false;
        self.stale_paint = true;
    }

    /// Lays the root out at the surface's size.
    fn lay_out(&mut self) -> Vec<Laid> {
        let cx = Cx {
            theme: &self.theme,
            s: self.scale,
        };
        let size = (self.size.0 as f32, self.size.1 as f32);
        self.layout.update(&self.root, size, &cx, &mut self.ui.text)
    }

    /// Keeps a scroll state for every scroll view laid out, with its new extent.
    fn sync_scrolls(&mut self) {
        let t = self.anim.t;
        let mut seen = HashSet::new();
        let mut followed = false;
        let areas: HashMap<u64, (f32, f32)> =
            self.fields.keys().filter_map(|&id| Some((id, self.field_extent(id)?))).collect();
        each(&self.root, &self.laid, &mut |el, i, _| {
            let field = areas.get(&self.laid[i].id).copied();
            let axis = el.scrolls().or(field.map(|_| Axis::Vertical));
            if let (Some(axis), Some(extent)) = (axis, self.laid[i].extent.or(field)) {
                let l = self.laid[i];
                let view = self.scrolls.entry(l.id).or_insert_with(|| ScrollView::new(axis));
                if view.axis != axis {
                    *view = ScrollView::new(axis);
                }
                let follow = el.follow_end && view.at_end();
                view.laid(l.rect, extent, t);
                if follow && !view.at_end() {
                    view.to(view.max, None, t);
                    followed = true;
                }
                seen.insert(l.id);
            }
        });
        self.scrolls.retain(|id, _| seen.contains(id));
        self.followed |= followed;
    }

    /// Keeps the elements of `old` that left with a transition (their parent stays) as ghosts.
    fn leave(&mut self, mut old: El<S>, ids: &HashSet<u64>) {
        let leaving: HashSet<u64> = self
            .laid
            .iter()
            .zip(&self.targets)
            .filter(|(l, t)| {
                t.transition.is_some()
                    && !ids.contains(&l.id)
                    && l.parent.is_some_and(|p| ids.contains(&self.laid[p].id))
            })
            .map(|(l, _)| l.id)
            .collect();
        if leaving.is_empty() {
            return;
        }
        let mut taken = Vec::new();
        take(&mut old, &self.laid, &mut 0, &leaving, &mut taken);
        for (start, index, el) in taken {
            self.exit(start, index, el);
        }
    }

    /// Keeps `el`, which left from place `index` among its parent's children and was laid out
    /// at `start`, drawn as a ghost until its exit settles (if it has a transition).
    fn exit(&mut self, start: usize, index: usize, el: El<S>) {
        let len = count(&el);
        let me = self.laid[start];
        let (Some(parent), Some(shown)) = (me.parent, self.poses.get(start..start + len)) else {
            return;
        };
        if !self.anim.leave(me.id) {
            return;
        }
        let origin = self.child_origin(parent);
        let mut frozen: Vec<Pose> = shown
            .iter()
            .map(|p| Pose {
                local: p.local.offset(-origin.x, -origin.y),
                ..p.clone()
            })
            .collect();
        // It leaves from its own opacity, not from wherever its entrance was.
        frozen[0].opacity = self.targets[start].opacity;
        self.ghosts.push(Ghost {
            id: me.id,
            parent: self.laid[parent].id,
            index,
            el,
            laid: self.laid[start..start + len].to_vec(),
            frozen,
        });
    }

    /// Where the children of the element at `index` hang from now: its top-left, less its
    /// scroll offset.
    fn child_origin(&self, index: usize) -> Point {
        let local = self.poses[index].local;
        let by = self
            .scrolls
            .get(&self.laid[index].id)
            .map_or(Point::new(0.0, 0.0), |v| v.shift(self.anim.t));
        Point::new(local.left - by.x, local.top - by.y)
    }

    /// Diffs keyed elements across a re-layout into [`Runtime::changes`].
    fn diff(&mut self, before: HashMap<u64, (Key, Snapshot)>, mut after: HashMap<u64, (Key, Snapshot)>) {
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
        let t = self.anim.t;
        let mut scrolled = HashMap::new();
        let mut bars = HashMap::new();
        if !self.scrolls.is_empty() {
            for (i, l) in self.laid.iter().enumerate() {
                if let Some(view) = self.scrolls.get_mut(&l.id) {
                    let (offset, alpha) = (view.offset(t), view.bar(t, &self.theme));
                    view.shown = (offset, alpha);
                    scrolled.insert(i, view.shift(t));
                    bars.insert(
                        i,
                        Bar {
                            axis: view.axis,
                            offset,
                            max: view.max,
                            alpha,
                        },
                    );
                }
            }
        }
        self.poses = self.anim.poses(&self.laid, &self.targets, &scrolled);
        let ghosts: Vec<Vec<Pose>> = if self.ghosts.is_empty() {
            Vec::new()
        } else {
            let at: HashMap<u64, usize> = self.laid.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
            self.ghosts
                .iter()
                .map(|g| {
                    at.get(&g.parent)
                        .map(|&p| {
                            // Ghosts in a scroll view move with its content.
                            let by = scrolled.get(&p).copied().unwrap_or(Point::new(0.0, 0.0));
                            let parent = Pose {
                                local: self.poses[p].local.offset(-by.x, -by.y),
                                ..self.poses[p].clone()
                            };
                            self.anim.ghost(g.id, &g.frozen, &parent)
                        })
                        .unwrap_or_default()
                })
                .collect()
        };
        let scene = Scene {
            ghosts: &self.ghosts,
            poses: &ghosts,
        };
        let entries = self.entries_by_index();
        let ghosted: HashSet<u64> = self.ghosts.iter().map(|g| g.id).collect();
        // An escaped overlay keeps its popup (at its open size) until its exit ends.
        let (layers, laid) = (&self.layers, &self.laid);
        self.lifted
            .retain(|id, _| ghosted.contains(id) || layers.iter().any(|l| l.id == *id && l.escaped));
        self.lifted.extend(
            layers
                .iter()
                .filter(|l| l.escaped)
                .filter_map(|l| laid.get(l.at).map(|e| (l.id, e.rect))),
        );
        let mut walk = Walk {
            scene: &scene,
            nav: Vec::with_capacity(self.nav.len()),
            semantics: Vec::with_capacity(self.semantics.len()),
            focus: self.ix.focus,
            bars: &bars,
            entries: &entries,
            relations: &self.relations,
            inert: None,
            lifted: &self.lifted,
            escaped: Vec::new(),
            fields: &self.fields,
            caret: self.caret_on().then_some(self.typing).flatten(),
            feedback: &self.feedback,
            t,
            actives: Vec::new(),
            pressable: 0,
            through: Vec::new(),
            clip: Rect::new(0.0, 0.0, self.size.0 as f32, self.size.1 as f32),
            modal: self.layers.iter().find(|l| l.kind == Kind::Dialog).map(|l| l.at),
            reduced: self.anim.reduced,
            hidden: !self.visible,
            spins: &mut self.spins,
            spinning: false,
        };
        self.caret_shown = walk.caret.is_some();
        let mut f = Frame::new(&mut self.ui.text, &self.theme, self.scale, &self.ix);
        if let Some(background) = self.background {
            let (w, h) = self.size;
            f.fill(Rect::new(0.0, 0.0, w as f32, h as f32), 0.0, background);
        }
        paint_el(&mut f, &mut walk, &self.laid, &self.poses, &self.root, &mut 0, true, None);
        let mut list = f.list;
        list.focus = self.ix.focus;
        let (mut nav, mut semantics, mut escaped) = (walk.nav, walk.semantics, walk.escaped);
        let (actives, through, spinning) = (walk.actives, walk.through, walk.spinning);
        self.spinning = spinning;
        // A modal dialog is all there is to point at, focus and read.
        if let Some(n) = walk.inert {
            list.hits.drain(..n);
            let left: HashSet<HitId> = list.hits.iter().map(|h| h.id).collect();
            nav.retain(|s| left.contains(&s.hit));
            for (.., hits) in &mut escaped {
                *hits = hits.start.saturating_sub(n)..hits.end.saturating_sub(n);
            }
        }
        // Active descendants by key, among the hits the list ends with.
        for (hit, key) in actives {
            let active = self.index_of_key(&key).map(|i| self.laid[i].hit);
            let Some(active) = active.filter(|a| list.hits.iter().any(|h| h.id == *a)) else { continue };
            for (_, semantic) in semantics.iter_mut().filter(|(id, _)| *id == hit) {
                if let Semantics::Detail(detail) = semantic {
                    detail.active_descendant = Some(active);
                }
            }
        }
        self.nav = nav;
        if semantics != self.semantics {
            self.semantics = semantics;
            self.semantics_sent = false;
        }
        self.pointing = (!through.is_empty()).then(|| DisplayList {
            ops: Vec::new(),
            hits: list.hits.iter().filter(|h| !through.contains(&h.id)).cloned().collect(),
            focus: list.focus,
        });
        self.window_list = self.cut_popups(&list, &escaped);
        self.damage = list.damage(&self.list);
        self.list = list;
        self.paints += 1;
        self.stale_paint = false;
    }

    /// Feeds one input event. Hover, press and focus changes repaint at the next frame
    /// without running the view; activations queue their handlers.
    pub fn handle(&mut self, event: &Event) {
        self.input(event);
        self.follow_tip();
    }

    fn input(&mut self, event: &Event) {
        match *event {
            Event::Press(..) | Event::Key(_) | Event::Activated => self.active = true,
            Event::Deactivated => self.active = false,
            _ => {}
        }
        match *event {
            #[cfg(windows)]
            Event::TextChanged => return self.rich_edit_changed(),
            #[cfg(windows)]
            Event::TextSubmit => return self.rich_edit_submit(),
            Event::Key(0x09) => return self.tab(self.mods.shift),
            Event::Key(vk) if self.key_down(KeyCode(vk)) => return,
            Event::FilesHover(p) => return self.files_over(Some(p.at)),
            Event::FilesLeave => return self.files_over(None),
            Event::FilesDropped(p) => return self.drop_files(p.at.x, p.at.y, crate::take_dropped_files()),
            Event::Wheel(p, delta) => {
                self.pointer = Some(p);
                // Scrolling closes a tooltip; its trigger shows none until the pointer leaves.
                self.hide_tip();
                if self.wheel_handler(p.at, delta) {
                    return;
                }
                return self.wheel(p.at, delta);
            }
            Event::Move(p) | Event::Press(p, _) | Event::Release(p, _) => self.pointer = Some(p),
            Event::Leave => self.pointer = None,
            _ => {}
        }
        if (!self.layers.is_empty() || !self.tips.is_empty()) && self.overlay_input(event) {
            return;
        }
        if let Event::Release(p, Button::Right) = *event {
            self.context_at(p.at);
        }
        let before = self.ix.clone();
        if let Response::Activate(hit) = self.ix.handle(self.pointing.as_ref().unwrap_or(&self.list), event) {
            self.queue.push(Action::Press(hit));
        }
        self.pointer_input(event);
        if !self.fields.is_empty()
            && matches!(event, Event::Press(..) | Event::Move(_) | Event::Release(..) | Event::CaptureLost | Event::Cancelled | Event::Leave)
        {
            self.sync_focus();
            self.field_pointer(event);
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
            self.follow_states();
            // Assistive technology moved the focus: show it.
            if self.ix.focus_visible
                && self.ix.focus != before.focus
                && let Some(index) = self.ix.focus.and_then(|hit| self.index_of_hit(hit))
            {
                self.reveal_index(index);
            }
        }
    }

    /// Runs the view with each virtual list's scroll place in reach ([`super::virtual_list`]).
    fn run_view(&mut self) -> El<S> {
        let places = self.virtual_places();
        super::virtualize::with_viewports(places, || (self.view)(&self.state))
    }

    /// Each keyed virtual list's offset and viewport now, design px.
    fn virtual_places(&self) -> HashMap<Key, (f32, f32)> {
        let mut places = HashMap::new();
        let (s, t) = (self.scale, self.anim.t);
        each(&self.root, &self.laid, &mut |el, i, _| {
            if el.virt.is_some()
                && let (Some(key), Some(view)) = (&el.key, self.laid.get(i).and_then(|l| self.scrolls.get(&l.id)))
            {
                places.insert(key.clone(), (view.offset(t) / s, view.viewport / s));
            }
        });
        places
    }

    /// Whether a virtual list's rows in sight left what it built.
    fn virtual_stale(&self) -> bool {
        let (s, t) = (self.scale, self.anim.t);
        let mut stale = false;
        each(&self.root, &self.laid, &mut |el, i, _| {
            if let Some(v) = &el.virt
                && let Some(view) = self.laid.get(i).and_then(|l| self.scrolls.get(&l.id))
            {
                stale |= v.stale(view.offset(t) / s, view.viewport / s);
            }
        });
        stale
    }

    /// Where each [`native`](super::native) element shows now: its name and its visible box in
    /// device px (cut to the scroll views around it), in paint order. Hosts place the surface's
    /// layers there after each frame ([`crate::Surface::place_native_layer`]); a name missing
    /// from the list is hidden (its element went away or scrolled out of sight).
    pub fn natives(&self) -> Vec<(Key, Rect)> {
        let flat = flatten(&self.root);
        let mut out = Vec::new();
        for (i, el) in flat.iter().enumerate() {
            let What::Native(name) = &el.what else { continue };
            let (Some(laid), Some(pose)) = (self.laid.get(i), self.poses.get(i)) else { continue };
            let mut rect = pose.rect;
            let mut at = laid.parent;
            while let Some(p) = at {
                if flat.get(p).is_some_and(|el| el.scrolls().is_some())
                    && let Some(view) = self.poses.get(p)
                {
                    rect = rect.intersect(view.rect);
                }
                at = self.laid.get(p).and_then(|l| l.parent);
            }
            if !rect.is_empty() && pose.opacity > 0.0 {
                out.push((name.clone(), rect));
            }
        }
        out
    }

    /// Files dragged from another app are over `at` (device px; `None`: they left): the
    /// innermost element there with [`on_drop_files`](super::El::on_drop_files) takes its
    /// [`dropping`](super::El::dropping) look.
    pub fn files_over(&mut self, at: Option<Point>) {
        let target = at.and_then(|p| self.drop_target_at(p));
        if target != self.drop_target {
            self.drop_target = target;
            self.follow_states();
            self.stale_paint = true;
        }
    }

    /// Files dropped at (`x`, `y`) device px: the innermost element there with
    /// [`on_drop_files`](super::El::on_drop_files) gets their paths. Hosts call it with
    /// [`take_dropped_files`](crate::take_dropped_files) on `Event::FilesDropped`
    /// ([`handle`](Self::handle) does).
    pub fn drop_files(&mut self, x: f32, y: f32, paths: Vec<std::path::PathBuf>) {
        let target = self.drop_target_at(Point::new(x, y));
        self.files_over(None);
        if let Some(i) = target.filter(|_| !paths.is_empty()) {
            self.queue.push(Action::Files(self.laid[i].hit, paths));
        }
    }

    /// The innermost element under `p` that takes dropped files.
    fn drop_target_at(&self, p: Point) -> Option<usize> {
        let flat = flatten(&self.root);
        (0..self.laid.len().min(flat.len()))
            .rev()
            .find(|&i| flat[i].on_files.is_some() && self.poses.get(i).is_some_and(|pose| pose.rect.contains(p)))
    }

    /// The modifier keys held now, for pointer and wheel events (`PointerEvent::mods`). Hosts
    /// pass them before each pointer event ([`run`](super::run) does).
    pub fn set_modifiers(&mut self, mods: Modifiers) {
        self.held = mods;
    }

    /// Whether the element hit `hit` takes raw pointer events.
    fn takes_pointer(&self, hit: HitId) -> bool {
        element(&self.root, &self.laid, hit).is_some_and(|el| el.on_pointer.is_some())
    }

    /// Feeds `on_pointer` elements: a press that begins on one holds the pointer until it is
    /// released; moves over one with nothing held hover it.
    fn pointer_input(&mut self, event: &Event) {
        use super::element::PointerPhase as Phase;
        match *event {
            Event::Press(p, button) | Event::DoubleClick(p, button) if self.capture.is_none() => {
                let clicks = if matches!(event, Event::DoubleClick(..)) { 2 } else { 1 };
                let Some(hit) = self.ix.hover.filter(|&h| self.takes_pointer(h)) else { return };
                self.capture = Some(Capture { hit, button, origin: p.at, last: p.at });
                self.push_pointer(hit, Phase::Down, button, p.at, p.at, p.at, clicks);
            }
            Event::Move(p) => match self.capture.as_mut() {
                Some(c) => {
                    let (hit, button, origin, last) = (c.hit, c.button, c.origin, c.last);
                    c.last = p.at;
                    self.push_pointer(hit, Phase::Move, button, p.at, last, origin, 1);
                }
                None => {
                    let hover = self.ix.hover.filter(|&h| self.takes_pointer(h));
                    let last = self.hovered_at.filter(|(h, _)| Some(*h) == hover).map_or(p.at, |(_, at)| at);
                    self.hovered_at = hover.map(|h| (h, p.at));
                    if let Some(hit) = hover {
                        self.push_pointer(hit, Phase::Hover, Button::Left, p.at, last, p.at, 1);
                    }
                }
            },
            Event::Release(p, button) if self.capture.is_some_and(|c| c.button == button) => {
                if let Some(c) = self.capture.take() {
                    self.push_pointer(c.hit, Phase::Up, button, p.at, c.last, c.origin, 1);
                }
            }
            Event::CaptureLost | Event::Cancelled | Event::Deactivated => {
                if let Some(c) = self.capture.take() {
                    self.push_pointer(c.hit, Phase::Cancel, c.button, c.last, c.last, c.origin, 1);
                }
            }
            Event::Leave => self.hovered_at = None,
            _ => {}
        }
    }

    /// Queues a pointer event for `hit` (device px in; design px out). A move right after a
    /// queued move of the same element merges into it, its deltas added.
    #[allow(clippy::too_many_arguments)]
    fn push_pointer(
        &mut self,
        hit: HitId,
        phase: super::element::PointerPhase,
        button: Button,
        at: Point,
        last: Point,
        origin: Point,
        clicks: u8,
    ) {
        use super::element::PointerPhase as Phase;
        let s = self.scale;
        let rect = self
            .laid
            .iter()
            .position(|l| l.hit == hit)
            .map(|i| self.poses.get(i).map_or(self.laid[i].rect, |p| p.rect))
            .unwrap_or_default();
        let design = |p: Point| Point::new(p.x / s, p.y / s);
        let mut event = super::element::PointerEvent {
            phase,
            button,
            at: design(at),
            local: Point::new((at.x - rect.left) / s, (at.y - rect.top) / s),
            delta: Point::new((at.x - last.x) / s, (at.y - last.y) / s),
            origin: design(origin),
            size: (rect.width() / s, rect.height() / s),
            mods: self.held,
            clicks,
        };
        if matches!(phase, Phase::Move | Phase::Hover)
            && let Some(Action::Pointer(h, prev)) = self.queue.last()
            && *h == hit
            && prev.phase == phase
        {
            event.delta = Point::new(event.delta.x + prev.delta.x, event.delta.y + prev.delta.y);
            self.queue.pop();
        }
        self.queue.push(Action::Pointer(hit, event));
    }

    /// Gives the wheel to the innermost element under the pointer with `on_wheel`; whether
    /// one took it.
    fn wheel_handler(&mut self, at: Point, delta: i16) -> bool {
        let s = self.scale;
        let mut index = self.ix.hover.and_then(|h| self.index_of_hit(h));
        let flat = if index.is_some() { flatten(&self.root) } else { Vec::new() };
        while let Some(i) = index.filter(|&i| i < self.laid.len()) {
            if flat.get(i).is_some_and(|el| el.on_wheel.is_some()) {
                let rect = self.poses.get(i).map_or(self.laid[i].rect, |p| p.rect);
                let event = super::element::WheelEvent {
                    at: Point::new(at.x / s, at.y / s),
                    local: Point::new((at.x - rect.left) / s, (at.y - rect.top) / s),
                    delta: f32::from(delta) / 120.0,
                    mods: self.held,
                };
                self.queue.push(Action::Wheel(self.laid[i].hit, event));
                return true;
            }
            index = self.laid[i].parent;
        }
        false
    }

    fn drag(&mut self, at: Point) {
        let Some(hit) = self.dragging else { return };
        // Where it is drawn now: a scroll view may have moved it.
        let Some(rect) = self
            .laid
            .iter()
            .position(|l| l.hit == hit)
            .map(|i| self.poses.get(i).map_or(self.laid[i].rect, |p| p.rect))
        else {
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

    // ---- Keys -------------------------------------------------------------------------------

    /// A key other than Tab: offered to the focused element and its ancestors (their
    /// `on_key`, then their range, focus group or scroll view), then the defaults. Whether it
    /// was used here (Enter, Space and Escape otherwise go on to the interaction state).
    fn key_down(&mut self, key: KeyCode) -> bool {
        if key == KeyCode::ESCAPE && self.tip.shown {
            self.hide_tip();
            return true;
        }
        // A focused field gets first refusal (Escape and Tab always go on).
        if key != KeyCode::ESCAPE && !self.fields.is_empty() {
            self.sync_focus();
            if self.field_key(key) == Some(true) {
                return true;
            }
        }
        let focused = self.ix.focus.and_then(|hit| self.index_of_hit(hit));
        let mut chain = Vec::new();
        let mut at = Some(focused.unwrap_or(0));
        while let Some(i) = at.filter(|&i| i < self.laid.len()) {
            chain.push(i);
            at = self.laid[i].parent;
        }
        let flat = flatten(&self.root);
        for &i in &chain {
            let Some(el) = flat.get(i).copied() else { continue };
            if let Some(handler) = &el.on_key {
                // Queued presses come first, as they arrived first.
                if !self.queue.is_empty() {
                    drop(flat);
                    self.run_handlers();
                    return self.key_down(key);
                }
                let mut tx = text::tx_with(self.mods);
                if handler(&mut self.state, key, &mut tx) == Handled::Yes {
                    self.stale_view = true;
                    self.apply(tx, true);
                    return true;
                }
            }
            if let Some(intent) = self.built_in(el, i, focused, key) {
                drop(flat);
                self.act(intent);
                return true;
            }
        }
        drop(flat);
        let step = match key {
            KeyCode::LEFT | KeyCode::UP => -1,
            KeyCode::RIGHT | KeyCode::DOWN => 1,
            KeyCode::ESCAPE => return self.escape(&chain),
            MENU => return self.context_key(&chain, focused),
            _ => return false,
        };
        self.step(step < 0);
        true
    }

    /// What element `el` (at `index`) does with `key` while focus is on `focused`.
    fn built_in(&self, el: &El<S>, index: usize, focused: Option<usize>, key: KeyCode) -> Option<Intent> {
        let on_it = focused == Some(index);
        let hit = self.laid[index].hit;
        if on_it
            && !el.sem.disabled
            && let (Some(range), Some(_)) = (el.sem.range, &el.on_step)
        {
            let value = range.value + self.stepped.get(&hit).copied().unwrap_or(0.0);
            let to = range_key(range, value, key)?;
            return Some(Intent::Step(hit, to - value));
        }
        if let Some(group) = el.group
            && let Some(focus) = self.ix.focus
            && let Some(stop) = self.nav.iter().find(|s| s.hit == focus && s.group == Some(index))
        {
            let (back, ahead) = match group.axis {
                Axis::Horizontal => (KeyCode::LEFT, KeyCode::RIGHT),
                Axis::Vertical => (KeyCode::UP, KeyCode::DOWN),
            };
            let members: Vec<HitId> = self.nav.iter().filter(|s| s.group == stop.group).map(|s| s.hit).collect();
            let at = members.iter().position(|&m| m == focus)?;
            let n = members.len();
            let to = match key {
                k if k == back => (at + n - 1) % n,
                k if k == ahead => (at + 1) % n,
                KeyCode::HOME => 0,
                KeyCode::END => n - 1,
                _ => return None,
            };
            return Some(Intent::Focus(members[to], group.follow));
        }
        if let Some(axis) = el.scrolls()
            && let Some(view) = self.scrolls.get(&self.laid[index].id)
        {
            let s = self.scale;
            let page = (view.viewport * 0.85).max(1.0);
            let (back, ahead) = match axis {
                Axis::Horizontal => (KeyCode::LEFT, KeyCode::RIGHT),
                Axis::Vertical => (KeyCode::UP, KeyCode::DOWN),
            };
            let to = match key {
                KeyCode::PAGE_UP => view.target() - page,
                KeyCode::PAGE_DOWN => view.target() + page,
                KeyCode::SPACE if on_it => view.target() + page,
                KeyCode::HOME if on_it => 0.0,
                KeyCode::END if on_it => view.max,
                k if on_it && k == back => view.target() - LINE * s,
                k if on_it && k == ahead => view.target() + LINE * s,
                _ => return None,
            };
            return Some(Intent::Scroll(index, to));
        }
        None
    }

    fn act(&mut self, intent: Intent) {
        match intent {
            Intent::Step(hit, delta) => {
                if delta != 0.0 {
                    *self.stepped.entry(hit).or_default() += delta;
                    self.queue.push(Action::Step(hit, delta));
                }
            }
            Intent::Focus(hit, press) => {
                self.focus_hit(hit);
                if press {
                    self.queue.push(Action::Press(hit));
                }
            }
            Intent::Scroll(index, to) => {
                let spring = self.scroll_spring();
                if let Some(view) = self.scrolls.get_mut(&self.laid[index].id)
                    && view.to(to, spring, self.anim.t)
                {
                    self.scrolling = true;
                    self.stale_paint = true;
                }
            }
        }
    }

    /// Moves keyboard focus to `hit`, shows its ring and scrolls it into view.
    fn focus_hit(&mut self, hit: HitId) {
        if self.ix.focus != Some(hit) || !self.ix.focus_visible {
            self.ix.focus = Some(hit);
            self.ix.focus_visible = true;
            self.stale_paint = true;
        }
        if let Some(index) = self.index_of_hit(hit) {
            self.reveal_index(index);
        }
    }

    /// Moves keyboard focus to the next (or, `back`, previous) focus stop (Tab), wrapping. A
    /// focus group is one stop: Tab lands on its selected member, else its first (its last
    /// going back). A stop hidden in a scroll view scrolls into view.
    pub fn tab(&mut self, back: bool) {
        self.step(back);
        self.follow_tip();
    }

    fn step(&mut self, back: bool) {
        let mut stops: Vec<Stop> = Vec::with_capacity(self.nav.len());
        // An open menu or dialog keeps focus inside.
        let trap = self.trap().map(Layer::range);
        for stop in self.nav.iter().filter(|s| trap.as_ref().is_none_or(|r| r.contains(&s.index))) {
            match stop.group {
                None => stops.push(*stop),
                Some(g) if stops.iter().any(|s| s.group == Some(g)) => {}
                Some(g) => {
                    let mut members = self.nav.iter().filter(|s| s.group == Some(g));
                    let entry = members.clone().find(|s| s.selected);
                    let edge = if back { members.next_back() } else { members.next() };
                    stops.extend(entry.or(edge).copied());
                }
            }
        }
        if stops.is_empty() {
            return;
        }
        let current = self.ix.focus.and_then(|focus| {
            let group = self.nav.iter().find(|s| s.hit == focus).and_then(|s| s.group);
            stops
                .iter()
                .position(|s| s.hit == focus || group.is_some() && s.group == group)
        });
        let n = stops.len();
        let next = match (current, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(at), false) => (at + 1) % n,
            (Some(at), true) => (at + n - 1) % n,
        };
        self.focus_hit(stops[next].hit);
    }

    /// Assistive technology's request on a hit item (a slider's increment, a new value):
    /// whether it was used. The window host routes `Surface::take_accessibility_action` here.
    pub fn accessibility_action(&mut self, hit: HitId, action: &AccessibilityAction) -> bool {
        if let AccessibilityAction::SetText(text) = action {
            return self.field_set_text(hit, text);
        }
        let Some(el) = element(&self.root, &self.laid, hit) else {
            return false;
        };
        let (Some(range), Some(_), false) = (el.sem.range, &el.on_step, el.sem.disabled) else {
            return false;
        };
        let value = range.value + self.stepped.get(&hit).copied().unwrap_or(0.0);
        let to = match *action {
            AccessibilityAction::Increment => value + range.step,
            AccessibilityAction::Decrement => value - range.step,
            AccessibilityAction::SetNumber(n) if n.is_finite() => n,
            _ => return false,
        };
        self.act(Intent::Step(hit, to.clamp(range.min, range.max) - value));
        true
    }

    // ---- Scrolling --------------------------------------------------------------------------

    /// The spring scrolling glides along now (`None`: it lands at once).
    fn scroll_spring(&self) -> Option<crate::Spring> {
        (!self.anim.reduced).then_some(self.theme.motion.scroll)
    }

    /// The wheel at `at` (device px): the innermost scroll view under it that can move takes
    /// it. A notch glides; a touchpad's small deltas move at once.
    fn wheel(&mut self, at: Point, delta: i16) {
        let notch = delta % 120 == 0;
        let by = -f32::from(delta) / 120.0 * NOTCH * self.scale;
        let spring = self.scroll_spring().filter(|_| notch);
        let t = self.anim.t;
        let under: Vec<u64> = self
            .laid
            .iter()
            .enumerate()
            .filter(|(i, l)| {
                self.scrolls.contains_key(&l.id) && self.poses.get(*i).is_some_and(|p| p.rect.contains(at))
            })
            .map(|(_, l)| l.id)
            .collect();
        for id in under.into_iter().rev() {
            if let Some(view) = self.scrolls.get_mut(&id)
                && view.by(by, spring, t)
            {
                self.scrolling = true;
                self.stale_paint = true;
                return;
            }
        }
    }

    /// Scrolls every scroll view around the element at `index` just enough to show it,
    /// innermost first.
    fn reveal_index(&mut self, index: usize) {
        let spring = self.scroll_spring().filter(|_| self.mode == Mode::Animate);
        let t = self.anim.t;
        // Within this of the content's ends, the content's padding shows too.
        let slack = (self.theme.metrics.pad * self.scale).round();
        let mut rect = self.laid[index].rect;
        let mut at = self.laid[index].parent;
        while let Some(p) = at {
            let l = self.laid[p];
            if let Some(view) = self.scrolls.get_mut(&l.id) {
                let (start, end, origin, extent) = match (view.axis, l.extent.unwrap_or_default()) {
                    (Axis::Vertical, (_, h)) => (rect.top, rect.bottom, l.rect.top, h),
                    (Axis::Horizontal, (w, _)) => (rect.left, rect.right, l.rect.left, w),
                };
                let (mut start, mut end) = (start - origin, end - origin);
                if start <= slack {
                    start = 0.0;
                }
                if extent - end <= slack {
                    end = extent;
                }
                if view.reveal(start, end, spring, t) {
                    self.scrolling = true;
                    self.stale_paint = true;
                }
                let by = view.target();
                rect = match view.axis {
                    Axis::Vertical => rect.offset(0.0, -by),
                    Axis::Horizontal => rect.offset(-by, 0.0),
                };
            }
            at = l.parent;
        }
    }

    /// Scrolls every scroll view around the first element keyed `key` just enough to show
    /// it, once the layout is current (at the next frame).
    pub fn scroll_into_view(&mut self, key: &str) {
        self.reveal.push(key.into());
    }

    /// Where the first scroll view keyed `key` rests (device px along its axis): its offset
    /// once any glide settles.
    pub fn scroll_offset(&self, key: &str) -> Option<f32> {
        let index = self.index_of_key(key)?;
        self.scrolls.get(&self.laid[index].id).map(ScrollView::target)
    }

    // ---- Lookups ----------------------------------------------------------------------------

    fn index_of_key(&self, key: &str) -> Option<usize> {
        let mut found = None;
        each(&self.root, &self.laid, &mut |el, i, _| {
            if found.is_none() && el.key.as_deref() == Some(key) {
                found = Some(i);
            }
        });
        found
    }

    /// The element whose hit item `hit` is: its own, or one its kit painting pushed.
    fn index_of_hit(&self, hit: HitId) -> Option<usize> {
        self.laid
            .iter()
            .position(|l| l.hit == hit)
            .or_else(|| self.nav.iter().find(|s| s.hit == hit).map(|s| s.index))
    }

    // ---- State ------------------------------------------------------------------------------

    /// Changes the state outside a handler (a timer, IPC); the view runs at the next frame
    /// and the change animates where elements have motion.
    pub fn update(&mut self, change: impl FnOnce(&mut S)) {
        change(&mut self.state);
        self.stale_view = true;
    }

    /// Runs `f` on the state without running the view: for host bookkeeping the view doesn't
    /// show (draining a model's outgoing requests, landing a cache), returning what `f`
    /// returns. Anything the view depends on goes through [`update`](Self::update) instead,
    /// or the screen keeps the old picture until the view next runs.
    pub fn touch<R>(&mut self, f: impl FnOnce(&mut S) -> R) -> R {
        f(&mut self.state)
    }

    /// [`update`](Self::update) with no motion: everything lands on its target at once.
    pub fn update_instant(&mut self, change: impl FnOnce(&mut S)) {
        self.update(change);
        self.land_next();
    }

    /// State.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Resizes the surface (device px): re-lays out without running the view, at once.
    pub fn resize(&mut self, size: (u32, u32)) {
        if size != self.size {
            self.size = size;
            self.stale_layout = true;
            self.land_next();
        }
    }

    /// Device px per design px (a DPI change): re-lays out without running the view, at once.
    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.scale {
            self.scale = scale;
            self.stale_layout = true;
            self.land_next();
        }
    }

    /// A new look: every token resolves again; the view does not run. Colours animate where
    /// elements have motion.
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

    /// How fast animations run: 1 is real time, 0.25 a quarter speed, 0 holds them.
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale.max(0.0);
    }

    /// Time scale.
    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }

    /// The system's reduced-motion setting ([`super::reduced_motion`]; the window host sets
    /// it): spatial travel snaps, while opacity, colour and presence take a short fade.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        if reduced != self.anim.reduced {
            self.anim.reduced = reduced;
            // Spinners stop or start.
            self.stale_paint = true;
        }
    }

    /// Reduced motion.
    pub fn reduced_motion(&self) -> bool {
        self.anim.reduced
    }

    /// Whether anything is still moving, leaving or scrolling (a scrollbar waiting to fade
    /// counts): the host keeps frames coming while it is, and none once it settles.
    pub fn animating(&self) -> bool {
        self.anim.moving() || !self.ghosts.is_empty() || self.scrolling || self.feedback.moving() || self.spinning
    }

    /// Whether the window shows (the host says when it is hidden or, where the platform tells,
    /// occluded): spinners ([`El::spin`]) hold still while it doesn't, and ask for no frames.
    pub fn set_visible(&mut self, visible: bool) {
        if visible != self.visible {
            self.visible = visible;
            self.stale_paint = true;
        }
    }

    /// Whether the window shows, as [`Runtime::set_visible`] last said.
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Scale.
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

    /// What assistive technology reads beyond each hit item's name (ranges, values,
    /// descriptions, selection, expansion), by hit, as of the latest paint.
    pub fn semantics(&self) -> &[(HitId, Semantics)] {
        &self.semantics
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

    /// Pointer move.
    pub fn pointer_move(&mut self, x: f32, y: f32) {
        self.handle(&Event::Move(pointer(x, y)));
    }

    /// Pointer down.
    pub fn pointer_down(&mut self, x: f32, y: f32) {
        self.handle(&Event::Press(pointer(x, y), Button::Left));
    }

    /// Pointer up.
    pub fn pointer_up(&mut self, x: f32, y: f32) {
        self.handle(&Event::Release(pointer(x, y), Button::Left));
    }

    /// The secondary button pressed and released at `(x, y)`: a context menu.
    pub fn right_click(&mut self, x: f32, y: f32) {
        self.handle(&Event::Press(pointer(x, y), Button::Right));
        self.handle(&Event::Release(pointer(x, y), Button::Right));
    }

    /// The wheel turned by `delta` (120 a notch, positive away from the user: up) at `(x, y)`.
    pub fn wheel_at(&mut self, x: f32, y: f32, delta: i16) {
        self.handle(&Event::Wheel(pointer(x, y), delta));
    }

    /// A key going down (a virtual-key code: Enter 0x0D, Space 0x20, arrows, Escape; see
    /// [`KeyCode`]).
    pub fn key(&mut self, vk: u16) {
        self.key_with(vk, Modifiers::NONE);
    }

    /// A key going down with modifiers held (Ctrl+A, Shift+Left, Shift+Tab): what
    /// [`El::on_key`] handlers read through [`Tx::modifiers`], and what text fields edit by.
    /// The window host sends every key this way.
    pub fn key_with(&mut self, vk: u16, mods: Modifiers) {
        self.mods = mods;
        self.handle(&Event::Key(vk));
        self.mods = Modifiers::NONE;
    }

    /// Presses and releases the pointer at the centre of the element keyed `key`, where it is
    /// drawn now; `false` if there is none.
    pub fn click(&mut self, key: &str) -> bool {
        let Some(at) = self.presented(key).map(|s| s.rect.center()) else {
            return false;
        };
        self.pointer_down(at.x, at.y);
        self.pointer_up(at.x, at.y);
        true
    }

    /// The laid-out rect (device px) of the first element keyed `key`: where it settles,
    /// before any scroll view moves it.
    pub fn find(&self, key: &str) -> Option<Rect> {
        self.index_of_key(key).map(|i| self.laid[i].rect)
    }

    /// What the first element keyed `key` shows now.
    pub fn presented(&self, key: &str) -> Option<Shown> {
        self.tree().into_iter().find(|s| s.key.as_deref() == Some(key))
    }

    /// Every element as shown now, in paint order (leaving elements are not in it).
    pub fn tree(&self) -> Vec<Shown> {
        let mut out = Vec::with_capacity(self.laid.len());
        each(&self.root, &self.laid, &mut |el, i, path| {
            let laid = self.laid[i];
            let pose = self.poses.get(i);
            let hit = el.hit();
            let enabled = el.interactive() && !el.sem.disabled;
            let sem = &el.sem;
            out.push(Shown {
                key: el.key.clone(),
                path: path.to_owned(),
                role: hit.then(|| role_of(el)),
                label: hit.then(|| label_of(el)),
                rect: pose.map_or(laid.rect, |p| p.rect),
                fill: pose.map_or_else(|| el.look.fill.map(|p| p.resolve(&self.theme)), |p| p.fill),
                text: match &el.what {
                    What::Text { text, .. } => Some(text.clone()),
                    What::Field(f) if !f.secure => self.field_shown(laid.id).map(|(t, _)| t.into()),
                    _ => None,
                },
                selection: el.field().filter(|f| !f.secure).and_then(|_| self.field_shown(laid.id)).map(|(_, s)| s),
                checked: sem.checked,
                state: if enabled { self.ix.state(laid.hit, true) } else { State::Rest },
                focused: self.ix.focus == Some(laid.hit),
                opacity: pose.map_or(el.look.opacity, |p| p.opacity),
                range: sem.range.map(|r| (r.min, r.max, r.value)),
                value: sem.value.clone(),
                description: sem.description.clone(),
                selected: sem.selected,
                expanded: sem.expanded.or(self.relations.get(&laid.id).and_then(|r| r.expanded)),
                scroll: self.scrolls.get(&laid.id).map(|v| v.offset(self.anim.t)),
                invalid: sem.invalid,
                active_descendant: sem.active.clone(),
            });
        });
        out
    }

    /// Shows the latest frame on `surface` at `origin` (screen px; app windows ignore it),
    /// with its accessibility values, less the overlays drawn outside the window
    /// ([`popups`](Self::popups)). The surface draws only what differs from what it shows.
    pub fn present(&mut self, surface: &mut Surface, origin: (i32, i32)) -> Result<bool> {
        if !self.semantics_sent {
            surface.set_accessibility_semantics(self.semantics.clone());
            self.semantics_sent = true;
        }
        let list = self.window_list.clone().unwrap_or_else(|| self.list.clone());
        surface.present(&mut self.ui, origin, self.size, list)
    }

    /// The latest frame drawn headless, through the same software target as a surface.
    pub fn snap(&mut self) -> Result<Image> {
        render(&mut self.ui, &self.list, self.size.0, self.size.1)
    }
}

/// What a built-in key behaviour does.
enum Intent {
    /// Step a range by this much.
    Step(HitId, f64),
    /// Move focus within a group (and press there).
    Focus(HitId, bool),
    /// Scroll the view at this place to this offset.
    Scroll(usize, f32),
}

/// Where `key` takes a range showing `value`, or `None` for a key it doesn't use.
fn range_key(r: Range, value: f64, key: KeyCode) -> Option<f64> {
    let page = ((r.max - r.min) / 10.0 / r.step).round().max(1.0) * r.step;
    let to = match key {
        KeyCode::RIGHT | KeyCode::UP => value + r.step,
        KeyCode::LEFT | KeyCode::DOWN => value - r.step,
        KeyCode::PAGE_UP => value + page,
        KeyCode::PAGE_DOWN => value - page,
        KeyCode::HOME => r.min,
        KeyCode::END => r.max,
        _ => return None,
    };
    Some(to.clamp(r.min, r.max))
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

/// Every element in pre-order: the same places as the laid-out list.
fn flatten<S>(root: &El<S>) -> Vec<&El<S>> {
    fn walk<'a, S>(el: &'a El<S>, out: &mut Vec<&'a El<S>>) {
        out.push(el);
        for child in &el.children {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

/// Visits every element in pre-order with its place in `laid` and its path.
fn each<S>(root: &El<S>, laid: &[Laid], visit: &mut impl FnMut(&El<S>, usize, &str)) {
    fn walk<S>(
        el: &El<S>,
        laid: &[Laid],
        i: &mut usize,
        path: &mut String,
        visit: &mut impl FnMut(&El<S>, usize, &str),
    ) {
        if *i >= laid.len() {
            return;
        }
        let me = *i;
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

/// How many elements `el` holds, itself included.
fn count<S>(el: &El<S>) -> usize {
    1 + el.children.iter().map(count).sum::<usize>()
}

/// Moves the subtrees whose identity is in `leaving` out of `el` (laid out as `laid`, from
/// `*i`), with their place in `laid` and among their siblings.
fn take<S>(el: &mut El<S>, laid: &[Laid], i: &mut usize, leaving: &HashSet<u64>, out: &mut Vec<(usize, usize, El<S>)>) {
    *i += 1;
    for (index, child) in el.children.iter_mut().enumerate() {
        let start = *i;
        if laid.get(start).is_some_and(|l| leaving.contains(&l.id)) {
            *i += count(child);
            out.push((start, index, std::mem::replace(child, super::el())));
        } else {
            take(child, laid, i, leaving, out);
        }
    }
}

fn keyed<S>(root: &El<S>, laid: &[Laid], theme: &Theme) -> HashMap<u64, (Key, Snapshot)> {
    let mut out = HashMap::new();
    each(root, laid, &mut |el, i, _| {
        if let Some(key) = &el.key {
            let snapshot = Snapshot {
                rect: laid[i].rect,
                fill: el.look.fill.map(|p| p.resolve(theme)),
                opacity: el.look.opacity,
            };
            out.insert(laid[i].id, (key.clone(), snapshot));
        }
    });
    out
}

/// Each laid-out element's targets, in the same order. Motion and `instant` reach down the
/// subtree. Elements in `within` (identities) take their focus-within look.
fn targets<S>(root: &El<S>, laid: &[Laid], theme: &Theme, s: f32, on: &super::element::Active) -> Vec<Target> {
    type Cx<'a> = (&'a [Laid], &'a Theme, f32, &'a super::element::Active);
    fn walk<S>(el: &El<S>, cx: Cx, i: &mut usize, motion: Option<crate::Spring>, instant: bool, inherited: Option<Rgba>, out: &mut Vec<Target>) {
        let (laid, theme, s, on) = cx;
        let Some(&me) = laid.get(*i) else { return };
        *i += 1;
        let look = on.look(me.id, el.look, &el.states);
        let instant = instant || el.anim.instant;
        let motion = if instant {
            None
        } else {
            el.anim.motion.map(|m| m.resolve(theme)).or(motion)
        };
        let origin = me.parent.map_or(Point::new(0.0, 0.0), |p| Point::new(laid[p].rect.left, laid[p].rect.top));
        let r = me.rect;
        // Its own ink, else (inside a `cascade_ink` element) the ink it inherits, else the label.
        let own = look.color.map(|p| p.resolve(theme));
        let (ink, value) = match &el.what {
            What::Arc(a) => (own.unwrap_or_else(|| token::ACCENT.resolve(theme)), a.fraction),
            _ => (own.or(inherited).unwrap_or_else(|| token::LABEL.resolve(theme)), 0.0),
        };
        let passed = (el.cascade || inherited.is_some()).then_some(ink);
        out.push(Target {
            rel: [r.left - origin.x, r.top - origin.y, r.right - origin.x, r.bottom - origin.y],
            fill: look.fill.map(|p| p.resolve(theme)),
            fill_gradient: look.fill.and_then(|p| p.gradient()).map(Arc::new),
            border_gradient: look.border.and_then(|b| b.paint.gradient()).map(Arc::new),
            border: look.border.map(|b| b.paint.resolve(theme)),
            border_width: look.border.map_or(0.0, |b| (b.width.resolve(theme) * s).max(0.0)),
            edges: look.edges.map(|(top, bottom)| [top.resolve(theme), bottom.resolve(theme)]),
            ink,
            opacity: look.opacity,
            radius: look
                .radius
                .map_or(0.0, |r| (r.resolve(theme) * s).round())
                .min(r.width().min(r.height()) / 2.0)
                .max(0.0),
            shadow: if look.shadow.is_some() { 1.0 } else { 0.0 },
            value,
            angle: el.anim.rotate,
            motion,
            open: motion.unwrap_or(theme.motion.open),
            transition: el.anim.transition.filter(|_| !instant).map(|t| Transition {
                slide: (t.slide.0 * s, t.slide.1 * s),
                ..t
            }),
        });
        for child in &el.children {
            walk(child, cx, i, motion, instant, passed, out);
        }
    }
    let mut out = Vec::with_capacity(laid.len());
    walk(root, (laid, theme, s, on), &mut 0, None, false, None, &mut out);
    out
}

fn role_of<S>(el: &El<S>) -> Role {
    el.sem.role.unwrap_or(if let Some(f) = el.field() {
        match (f.secure, f.multiline) {
            (true, _) => Role::PasswordInput,
            (_, true) => Role::MultilineTextInput,
            _ => Role::TextInput,
        }
    } else if el.on_drag.is_some() || el.sem.range.is_some() {
        Role::Slider
    } else if el.scrolls().is_some() {
        Role::ScrollView
    } else if el.interactive() && el.sem.checked.is_some() {
        Role::Switch
    } else if el.interactive() {
        Role::Button
    } else {
        Role::Label
    })
}

fn label_of<S>(el: &El<S>) -> Arc<str> {
    let first = || el.first_text().cloned().filter(|_| el.scrolls().is_none());
    let first = || first().or_else(|| el.field().and_then(|f| f.placeholder.clone()));
    el.sem.label.clone().or_else(first).unwrap_or_else(|| "".into())
}

/// What `el` tells assistive technology beyond its name and role; `rel` is what it opens
/// (a trigger) or whether it is a modal panel.
fn semantics_of<S>(el: &El<S>, id: HitId, rel: Option<&Relation>, out: &mut Vec<(HitId, Semantics)>) {
    let s = &el.sem;
    let rel = rel.copied().unwrap_or_default();
    let expanded = s.expanded.or(rel.expanded);
    if let Some(r) = s.range {
        out.push((
            id,
            Semantics::Range {
                value: r.value,
                min: r.min,
                max: r.max,
                step: r.step,
            },
        ));
    }
    let detail = Detail {
        description: s.description.clone(),
        value: s.value.clone(),
        selected: s.selected,
        expanded,
        popup: rel.popup,
        controls: rel.controls,
        modal: rel.modal,
        invalid: s.invalid,
        // Keys resolve once the whole list is painted (`Runtime::paint`).
        active_descendant: None,
    };
    if detail != Detail::default() || s.active.is_some() {
        out.push((id, Semantics::Detail(detail)));
    }
}

/// The leaving elements to draw among the live ones, with their poses now.
struct Scene<'a, S> {
    ghosts: &'a [Ghost<S>],
    poses: &'a [Vec<Pose>],
}

/// What a paint walk gathers besides the list.
struct Walk<'a, S> {
    scene: &'a Scene<'a, S>,
    nav: Vec<Stop>,
    semantics: Vec<(HitId, Semantics)>,
    /// Kept in the list even when scrolled out of sight, so Enter and assistive technology
    /// still find it.
    focus: Option<HitId>,
    /// Scroll views by their place in the live `laid`.
    bars: &'a HashMap<usize, Bar>,
    /// Overlay entries by their place in the live `laid`.
    entries: &'a HashMap<usize, Entry>,
    relations: &'a HashMap<u64, Relation>,
    /// Hits before this one are under a modal dialog.
    inert: Option<usize>,
    /// Overlays (live or leaving) drawn outside the window, by identity.
    lifted: &'a HashMap<u64, Rect>,
    /// Escaped entries: their identity, ops and hits.
    escaped: Vec<Escaped>,
    fields: &'a HashMap<u64, Field>,
    /// The field whose caret is drawn now.
    caret: Option<u64>,
    feedback: &'a Feedback,
    /// The animation clock.
    t: f64,
    /// Elements naming an active descendant: their hit and its key.
    actives: Vec<(HitId, Key)>,
    /// How many enabled controls the walk is inside.
    pressable: usize,
    /// Hits the pointer goes through (announced content inside a control).
    through: Vec<HitId>,
    // ---- Spinning (`El::spin`) -------------------------------------------------------
    /// What can show here: the window, cut to the scroll views around.
    clip: Rect,
    /// Elements before this place in `laid` are under an open dialog.
    modal: Option<usize>,
    /// Reduced motion: spinners rest at their own angle.
    reduced: bool,
    /// The window is hidden: spinners hold where they are.
    hidden: bool,
    /// Each spinner's turn when it last showed, by identity.
    spins: &'a mut HashMap<u64, f32>,
    /// Whether something visible spins (frames keep coming).
    spinning: bool,
}

/// A spinner's turn now, radians: moving along the clock while it can be seen (live, inside
/// the window and its scroll views, above any open dialog, the window shown, motion not
/// reduced), else holding where it last showed (at rest under reduced motion).
fn spin<S>(w: &mut Walk<S>, id: u64, index: usize, rect: Rect, live: bool, period: Duration) -> f32 {
    if w.reduced {
        return 0.0;
    }
    let seen = live && !w.hidden && w.modal.is_none_or(|m| index >= m) && !rect.is_empty() && rect.intersects(&w.clip);
    if !seen {
        return w.spins.get(&id).copied().unwrap_or(0.0);
    }
    w.spinning = true;
    let turns = (w.t / period.as_secs_f64()).fract() as f32;
    // To a thousandth of a radian: a still frame repaints nothing new.
    let angle = (turns * TAU * 1000.0).round() / 1000.0;
    w.spins.insert(id, angle);
    angle
}

/// An entry painted for a popup surface: its identity, ops and hits in the list.
type Escaped = (u64, std::ops::Range<usize>, std::ops::Range<usize>);

impl<S> Scene<'_, S> {
    /// Paints the ghosts that left `parent` from place `index` (and, `rest`, any after it).
    fn paint(&self, f: &mut Frame, w: &mut Walk<S>, parent: u64, index: usize, rest: bool) {
        for (g, poses) in self.ghosts.iter().zip(self.poses) {
            if g.parent == parent && (g.index == index || rest && g.index > index) {
                // Out of hit testing: whatever hits its kit painting pushed go again.
                let (ops, hits) = (f.list.ops.len(), f.list.hits.len());
                paint_el(f, w, &g.laid, poses, &g.el, &mut 0, false, None);
                f.list.hits.truncate(hits);
                // An overlay that left from a popup surface fades out there.
                if w.lifted.contains_key(&g.id) {
                    w.escaped.push((g.id, ops..f.list.ops.len(), hits..hits));
                }
            }
        }
    }
}

/// Paints `el` and its children (pre-order, matching `laid` and `poses`); `live` is false for
/// a leaving element, which takes no hits, wash or focus ring. `group` is the focus group
/// around it (its place in `laid`).
#[allow(clippy::too_many_arguments)]
fn paint_el<S>(
    f: &mut Frame,
    w: &mut Walk<S>,
    laid: &[Laid],
    poses: &[Pose],
    el: &El<S>,
    i: &mut usize,
    live: bool,
    group: Option<usize>,
) {
    let (Some(&me), Some(pose)) = (laid.get(*i), poses.get(*i)) else { return };
    let index = *i;
    *i += 1;
    let (theme, s, rect) = (f.theme, f.s, pose.rect);
    let start = f.list.ops.len();
    let hits = f.list.hits.len();
    let stops = w.nav.len();
    let entry = if live { w.entries.get(&index).copied() } else { None };
    let enabled = el.interactive() && !el.sem.disabled;
    let radius = pose.radius();
    let ink = pose.ink;
    let paints = matches!(el.what, What::Paint(_));
    let controls = live && el.interactive() && !paints;
    let scroll = el.scrolls();
    if live && el.hit() && !paints {
        f.list.hits.push(hit(el, me.hit, rect));
        // Announced content inside a control (a table's cell in its row) has its own hit, but
        // the pointer goes through it to the control.
        if w.pressable > 0 && !el.interactive() {
            w.through.push(me.hit);
        }
    }
    // A shadowed box keeps its ops while they fade in (popups size themselves by them).
    if pose.shadow > 0.0 || el.look.shadow.is_some() {
        // A shadow fading out after its box dropped it fades the default level.
        let states = [el.states.within, el.states.hover, el.states.press];
        let cast = el.look.shadow.or(states.iter().flatten().find_map(|w| w.shadow));
        let cast = cast.unwrap_or(super::element::Cast::Level(Default::default()));
        for layer in cast.layers(theme).iter().filter(|l| l.color.a > 0.0) {
            f.push(Op::SoftShadow {
                rect,
                radius,
                blur: layer.blur * s,
                offset: (layer.offset.0 * s, layer.offset.1 * s),
                spread: layer.spread * s,
                color: layer.color.with_alpha(layer.color.a * pose.shadow),
            });
        }
    }
    if let Some(gradient) = &pose.fill_gradient {
        f.push(Op::Gradient { rect, radius, width: 0.0, gradient: (**gradient).clone() });
    } else if let Some(fill) = pose.fill {
        f.fill(rect, radius, fill);
    }
    // Edges: the box's shape in the edge colour, clipped to one design px at its top and at
    // its bottom, so they follow the corners like CSS's `inset 0 1px 0`.
    if let Some([top, bottom]) = pose.edges {
        let px = (s * pose.xf.s).max(1.0).round();
        for (color, band) in [
            (top, Rect::new(rect.left, rect.top, rect.width(), px)),
            (bottom, Rect::new(rect.left, rect.bottom - px, rect.width(), px)),
        ] {
            if color.a > 0.0 {
                f.push(Op::Clipped { rect: band, ops: Arc::from([Op::Fill { rect, radius, color }]) });
            }
        }
    }
    // A scroll view takes focus for its keys, but is no button: no wash; nor is a field.
    if controls && scroll.is_none() && el.field().is_none() {
        let c = &theme.color;
        let wash = if enabled { w.feedback.wash(me.hit, w.t, c.hover, c.pressed) } else { Rgba::TRANSPARENT };
        f.fill(rect, radius, wash);
    }
    if let Some(gradient) = pose.border_gradient.as_ref().filter(|_| pose.border_width > 0.0) {
        let bw = (pose.border_width * pose.xf.s).max(1.0);
        f.push(Op::Gradient { rect: rect.inflate(-bw / 2.0), radius: (radius - bw / 2.0).max(0.0), width: bw, gradient: (**gradient).clone() });
    } else if let Some(color) = pose.border.filter(|c| c.a > 0.0 && pose.border_width > 0.0) {
        let bw = (pose.border_width * pose.xf.s).max(1.0);
        f.push(Op::Stroke {
            rect: rect.inflate(-bw / 2.0),
            radius: (radius - bw / 2.0).max(0.0),
            width: bw,
            color,
        });
    }
    // Text fits its laid-out width, so a box springing narrower never cuts it short.
    let width = me.rect.width();
    match &el.what {
        What::Box(_) => {}
        What::Native(_) => f.push(Op::Hole { rect }),
        What::Field(spec) => {
            if let Some(field) = w.fields.get(&me.id) {
                let focused = w.focus == Some(me.hit);
                let caret = (w.caret == Some(me.id)).then(|| token::CARET.resolve(theme));
                let bar = w.bars.get(&index).copied();
                let hint = token::TERTIARY.resolve(theme);
                let selection = if focused { token::SELECTION.resolve(theme) } else { Rgba::TRANSPARENT };
                let scroll_y = bar.map_or(0.0, |b| b.offset);
                #[cfg(windows)]
                let native = field.native.as_ref();
                #[cfg(not(windows))]
                let native: Option<&()> = None;
                match native {
                    #[cfg(windows)]
                    Some(native) => native::paint_rich_edit(f, field, native, spec.placeholder.as_deref(), caret, hint),
                    _ => super::field::paint(f, field, rect, spec.placeholder.as_deref(), caret, selection, hint, scroll_y),
                }
                if let Some(bar) = bar.filter(|_| live) {
                    let extent = field.look.as_ref().map(|l| (l.rect.width(), field.shape.height() + l.rect.height() - l.content.height()));
                    scrollbar(f, rect, extent, bar);
                }
                // What assistive technology reads and sets; a secure field tells nothing.
                if live && !spec.secure {
                    w.semantics.push((me.hit, Semantics::Text(field.ed.text.as_str().into())));
                    if let Some(text) = field.accessible(scroll_y) {
                        w.semantics.push((me.hit, Semantics::Field(Arc::new(text))));
                    }
                }
            }
        }
        What::Text { text, face, wrap: None } => {
            let style = face.style(theme, s);
            let fitted = f.text.fit(text, &style, width);
            let (w, h) = f.text.measure(&fitted, &style);
            let top = (rect.top + (rect.height() - h) / 2.0).round();
            let left = align_x(rect, w, face.align);
            f.push(Op::Text {
                rect: Rect::new(left, top, w, h),
                text: fitted,
                style,
                color: ink,
            });
        }
        What::Text { text, face, wrap: Some(lines) } => paragraph(f, rect, text, face, ink, *lines),
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
        What::Arc(a) => paint_arc(f, rect, a, pose.value, ink),
        What::Paint(painter) => {
            painter(f, rect, me.hit);
            if live && el.hit() && !f.list.hits[hits..].iter().any(|h| h.id == me.hit) {
                f.list.hits.push(hit(el, me.hit, rect));
            }
        }
    }
    if live {
        let selected = el.sem.selected == Some(true) || el.sem.checked == Some(true);
        for h in f.list.hits[hits..].iter().filter(|h| h.actionable) {
            w.nav.push(Stop {
                hit: h.id,
                index,
                group,
                selected,
            });
        }
        if el.hit() {
            semantics_of(el, me.hit, w.relations.get(&me.id), &mut w.semantics);
            if let Some(key) = &el.sem.active {
                w.actives.push((me.hit, key.clone()));
            }
        }
    }
    let inner = if el.group.is_some() { Some(index) } else { group };
    let outer = scroll.map(|_| std::mem::take(&mut f.list.ops));
    let (first_hit, first_stop) = (f.list.hits.len(), w.nav.len());
    let scene = w.scene;
    // What a scroll view's content can show is cut to it.
    let clip = w.clip;
    if scroll.is_some() {
        w.clip = w.clip.intersect(rect);
    }
    for (n, child) in el.children.iter().enumerate() {
        if live && !scene.ghosts.is_empty() {
            scene.paint(f, w, me.id, n, false);
        }
        w.pressable += usize::from(enabled && live);
        paint_el(f, w, laid, poses, child, i, live, inner);
        w.pressable -= usize::from(enabled && live);
    }
    w.clip = clip;
    if live && !scene.ghosts.is_empty() {
        scene.paint(f, w, me.id, el.children.len(), true);
    }
    if let Some(outer) = outer {
        let content = std::mem::replace(&mut f.list.ops, outer);
        f.push(Op::Clipped {
            rect,
            ops: content.into(),
        });
        // Hits are cut to the view; those out of sight go, except the focused one.
        let shown: Vec<Hit> = f.list.hits.split_off(first_hit);
        f.list.hits.extend(shown.into_iter().filter_map(|mut h| {
            let cut = h.rect.intersect(rect);
            if !cut.is_empty() {
                h.rect = cut;
            } else if w.focus == Some(h.id) {
                h.rect = Rect::new(cut.left.min(rect.right), cut.top.min(rect.bottom), 0.0, 0.0);
            } else {
                return None;
            }
            Some(h)
        }));
        // A view with focus stops inside is no Tab stop itself (it still takes a click's focus).
        if w.nav.len() > first_stop {
            w.nav.retain(|stop| stop.hit != me.hit || stop.index != index);
        }
        if let Some(bar) = w.bars.get(&index).filter(|_| live) {
            scrollbar(f, rect, me.extent, *bar);
        }
    }
    if controls && !el.quiet {
        focus_ring(f, rect, radius, w.feedback.ring(me.hit, w.t));
    }
    // A turn (its own and its spin's) at draw time: what it drew, and its hits' boxes.
    let angle = pose.angle + el.anim.spin.map_or(0.0, |period| spin(w, me.id, index, rect, live, period));
    if angle != 0.0 {
        let c = rect.center();
        let turn = super::rotate::Turn::new(Point::new(c.x.round(), c.y.round()), angle);
        if !turn.none() {
            let drawn = f.list.ops.split_off(start);
            f.list.ops.extend(turn.ops(drawn));
            for h in &mut f.list.hits[hits..] {
                h.rect = turn.bounds(h.rect);
            }
        }
    }
    if pose.opacity < 1.0 {
        fade(&mut f.list.ops[start..], pose.opacity);
    }
    if let Some(entry) = entry {
        match entry.kind {
            // A tooltip takes no pointer and no focus: its trigger describes it.
            Kind::Tooltip => {
                f.list.hits.truncate(hits);
                w.nav.truncate(stops);
            }
            Kind::Dialog => w.inert = Some(hits),
            _ => {}
        }
        if entry.escaped {
            w.escaped.push((me.id, start..f.list.ops.len(), hits..f.list.hits.len()));
        }
    }
}

/// A keyboard focus ring just outside `rect`, as the theme's [`crate::theme::FocusRing`] says,
/// `shown` of the way in (always pushed, transparent at 0). Coming in it fades up and grows
/// from the box's edge out to its gap; shown, it draws the same pixels as
/// [`Frame::focus_ring`] with the default ring. Keyboard focus shows it, and a field shows it
/// whenever it has focus (the caret is in it).
fn focus_ring(f: &mut Frame, rect: Rect, radius: f32, shown: f32) {
    let ring = f.theme.focus_ring;
    let width = f.px(ring.width).max(ring.width);
    let gap = (f.px(ring.gap) * shown).round();
    let alpha = if shown >= 1.0 { ring.opacity } else { (ring.opacity * shown * 255.0).round() / 255.0 };
    f.push(Op::Stroke {
        rect: rect.inflate(gap + width / 2.0),
        radius: radius + gap + width / 2.0,
        width,
        color: if alpha > 0.0 { f.theme.color.accent.with_alpha(alpha) } else { Rgba::TRANSPARENT },
    });
}

/// A scroll view's overlay scrollbar inside `rect`'s far edge, its thumb as long as the share
/// of the content that shows (always pushed, transparent while hidden).
fn scrollbar(f: &mut Frame, rect: Rect, extent: Option<(f32, f32)>, bar: Bar) {
    let m = f.theme.metrics;
    let (w, inset, least) = (f.px(m.scrollbar), f.px(m.scrollbar_inset), f.px(28.0));
    let (along, content) = match bar.axis {
        Axis::Vertical => (rect.height(), extent.map_or(0.0, |e| e.1)),
        Axis::Horizontal => (rect.width(), extent.map_or(0.0, |e| e.0)),
    };
    let track = (along - 2.0 * inset).max(0.0);
    let thumb = if content > 0.0 { (track * along / content).max(least).min(track) } else { track };
    let at = if bar.max > 0.0 { (track - thumb) * (bar.offset / bar.max) } else { 0.0 };
    let thumb_rect = match bar.axis {
        Axis::Vertical => Rect::new(rect.right - inset - w, (rect.top + inset + at).round(), w, thumb.round()),
        Axis::Horizontal => Rect::new((rect.left + inset + at).round(), rect.bottom - inset - w, thumb.round(), w),
    };
    let color = f.theme.color.scrollbar;
    let shown = bar.alpha > 0.0 && bar.max > 0.0;
    f.fill(
        thumb_rect,
        w / 2.0,
        if shown { color.with_alpha(color.a * bar.alpha) } else { Rgba::TRANSPARENT },
    );
}

/// An [`super::arc`]: its track, then `value` of its span in `ink`, round-capped, in a circle
/// fitting `rect` (centred on whole pixels, so the quarter clips land on pixel edges).
fn paint_arc(f: &mut Frame, rect: Rect, a: &ArcLook, value: f32, ink: Rgba) {
    let width = f.px(a.stroke.resolve(f.theme)).max(1.0);
    let c = rect.center();
    let center = Point::new(c.x.round(), c.y.round());
    let radius = ((rect.width().min(rect.height()) - width) / 2.0).max(0.0);
    if radius <= 0.0 {
        return;
    }
    let span = a.span * TAU;
    if let Some(track) = a.track {
        stroke_arc(f, center, radius, width, a.start, span, track.resolve(f.theme));
    }
    stroke_arc(f, center, radius, width, a.start, value.clamp(0.0, 1.0) * span, ink);
}

/// A clockwise arc of `sweep` radians from `start` radians past twelve o'clock, with round
/// caps unless it closes.
fn stroke_arc(f: &mut Frame, center: Point, radius: f32, width: f32, start: f32, sweep: f32, color: Rgba) {
    let sweep = sweep.min(TAU);
    if sweep <= 0.0 || color.a <= 0.0 {
        return;
    }
    f.push(Op::Arc {
        center,
        radius,
        start,
        sweep,
        width,
        color,
    });
    if sweep >= TAU - 1e-4 {
        return;
    }
    let end = start + sweep;
    // Round caps. On a quarter only the half beyond the arc's end draws (the end's edge is
    // radial, so the quarter's axis cuts it exactly), so a translucent stroke doesn't darken
    // where cap and stroke overlap; elsewhere the whole disc.
    let point = |angle: f32| Point::new(center.x + radius * angle.sin(), center.y - radius * angle.cos());
    let half = width / 2.0 + 1.0;
    // The half-plane before an arc that starts on quarter `q`, around its start `at`.
    let before = |at: Point, q: u8| match q % 4 {
        0 => Rect::new(at.x - half, at.y - half, half, 2.0 * half),
        1 => Rect::new(at.x - half, at.y - half, 2.0 * half, half),
        2 => Rect::new(at.x, at.y - half, half, 2.0 * half),
        _ => Rect::new(at.x - half, at.y, 2.0 * half, half),
    };
    let disc = |at: Point| Op::Fill {
        rect: Rect::new(at.x - width / 2.0, at.y - width / 2.0, width, width),
        radius: width / 2.0,
        color,
    };
    // The quarter an angle lies on, if it does.
    let quarter = |angle: f32| {
        let k = angle / FRAC_PI_2;
        ((k - k.round()).abs() < 1e-4).then(|| (k.round() as i64).rem_euclid(4) as u8)
    };
    let at = point(start);
    match quarter(start) {
        Some(q) => f.push(Op::Clipped {
            rect: before(at, q),
            ops: vec![disc(at)].into(),
        }),
        None => f.push(disc(at)),
    }
    let at = point(end);
    match quarter(end) {
        // Past an end on quarter k is before a start on the opposite quarter.
        Some(q) => f.push(Op::Clipped {
            rect: before(at, q + 2),
            ops: vec![disc(at)].into(),
        }),
        None => f.push(disc(at)),
    }
}

/// Multiplies the alpha of everything `ops` draw by `by`.
fn fade(ops: &mut [Op], by: f32) {
    for op in ops {
        match op {
            Op::Gradient { gradient, .. } => gradient.fade(by),
            Op::Continuous { color, .. }
            | Op::Emphasis { color, .. }
            | Op::Arc { color, .. }
            | Op::Fill { color, .. }
            | Op::Stroke { color, .. }
            | Op::Line { color, .. }
            | Op::Text { color, .. }
            | Op::Icon { color, .. }
            | Op::Shadow { color, .. }
            | Op::SoftShadow { color, .. } => *color = color.with_alpha(color.a * by),
            Op::Image { opacity, .. } | Op::RoundedImage { opacity, .. } => *opacity *= by,
            Op::Clipped { ops, .. } | Op::RoundedClipped { ops, .. } => fade(Arc::make_mut(ops), by),
            Op::TextField { .. } | Op::Clip { .. } | Op::Hole { .. } | Op::Unclip => {}
        }
    }
}

/// `el`'s hit item at `rect`: announced (as disabled when it is), and actionable, taking
/// hover, press and focus, only when it has a handler or is `focusable`.
fn hit<S>(el: &El<S>, id: HitId, rect: Rect) -> Hit {
    Hit {
        id,
        rect,
        role: role_of(el),
        label: label_of(el),
        enabled: !el.sem.disabled,
        actionable: el.interactive() && !el.sem.disabled,
        checked: el.sem.checked,
    }
}

/// Where a line `w` wide starts in `rect` for `align`, on a whole device pixel.
fn align_x(rect: Rect, w: f32, align: super::element::TextAlign) -> f32 {
    use super::element::TextAlign;
    match align {
        TextAlign::Start => rect.left,
        TextAlign::Center => (rect.left + (rect.width() - w) / 2.0).round(),
        TextAlign::End => (rect.right - w).round(),
    }
}

/// Wrapped text in `face`: at most `lines` lines of `rect`'s width, each centred in the
/// face's line height and placed across by its alignment.
fn paragraph(f: &mut Frame, rect: Rect, text: &str, face: &super::element::Face, color: Rgba, lines: usize) {
    let style = face.style(f.theme, f.s);
    let line = face.line(f.theme, f.s);
    let (_, h) = f.text.measure("Ag", &style);
    let lead = ((line - h) / 2.0).round();
    let mut top = rect.top.round();
    for text in f.text.lines(text, &style, rect.width(), lines) {
        let (w, h) = f.text.measure(&text, &style);
        f.push(Op::Text {
            rect: Rect::new(align_x(rect, w, face.align), top + lead, w, h),
            text,
            style: style.clone(),
            color,
        });
        top += line;
    }
}
