//! The runtime's overlay layer ([`super::super::overlay`]). After each view run the overlays
//! the view declared are lifted out of the tree, and the open ones become the root's last
//! children ("entries"), so layout, animation, presence, hit testing, keys and focus treat
//! them like any element. A relayout then places each entry against its anchor by moving its
//! laid-out subtree (Taffy never sees the placement), and paint gives a modal dialog the only
//! hits, keeps tooltips out of hit testing and cuts escaped entries out for popup surfaces.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use accesskit::HasPopup;

use super::super::element::{Align, Axis, El, KeyCode, Len, Transition};
use super::super::layout::{Laid, ROOT, child_id, hit_of};
use super::super::overlay::{Anchor, Dismiss, Kind, Overlay, Place, Side, TOOLTIP_DELAY, place, tooltip_el};
use super::super::token;
use super::{Action, Runtime, count, each, flatten};
use crate::{DisplayList, HitId, Point, Rect};

/// An open overlay. Its entry is the root's child at `entries + its place in layers`.
pub(super) struct Layer<S> {
    pub id: u64,
    pub kind: Kind,
    /// The element that declared it.
    pub trigger: Option<u64>,
    pub anchor: Anchor,
    pub side: Option<Side>,
    pub align: Align,
    pub offset: Len,
    pub match_width: bool,
    pub focus: bool,
    pub on_dismiss: Option<Dismiss<S>>,
    /// Its entry's place in `laid`, and how many elements it holds (set by placement).
    pub at: usize,
    pub len: usize,
    /// Drawn in a popup surface of its own.
    pub escaped: bool,
}

impl<S> Layer<S> {
    /// The panel's place in `laid`: the entry, or a dialog's panel inside its backdrop.
    pub fn panel(&self) -> usize {
        self.at + usize::from(self.kind == Kind::Dialog)
    }

    pub fn range(&self) -> Range<usize> {
        self.at..self.at + self.len
    }
}

/// What a trigger or panel tells assistive technology beyond its own semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Relation {
    pub popup: Option<HasPopup>,
    pub expanded: Option<bool>,
    pub controls: Option<HitId>,
    pub modal: bool,
}

/// The tooltip's state: whose it is, when it opens, whether it shows.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Tip {
    pub trigger: Option<u64>,
    pub due: Option<Duration>,
    pub shown: bool,
    /// Pressed or Escaped: none until the pointer and focus move to another trigger.
    pub quiet: bool,
}

/// How paint treats an overlay entry (by its place in `laid`).
#[derive(Debug, Clone, Copy)]
pub(super) struct Entry {
    pub kind: Kind,
    pub escaped: bool,
}

/// An overlay drawn outside the window: its list in its own pixels, and where it goes
/// (device px from the window's top-left).
#[derive(Debug, Clone, PartialEq)]
pub struct Popup {
    /// The overlay's identity, stable while it stays open.
    pub id: u64,
    pub rect: Rect,
    pub list: DisplayList,
}

/// Takes the overlays out of `el` (identity `id`) and its subtree, in pre-order, with their
/// trigger's identity; gathers tooltips, relations and (with `ids`) every identity seen.
#[allow(clippy::type_complexity)]
fn collect<S>(
    el: &mut El<S>,
    id: u64,
    found: &mut VecDeque<(u64, usize, Overlay<S>)>,
    tips: &mut Vec<(u64, Arc<str>)>,
    relations: &mut HashMap<u64, Relation>,
    ids: &mut Option<HashSet<u64>>,
) {
    if let Some(ids) = ids {
        ids.insert(id);
    }
    if let Some(popup) = el.popup {
        relations.insert(
            id,
            Relation {
                popup: Some(popup),
                expanded: Some(false),
                ..Relation::default()
            },
        );
    }
    for (nth, overlay) in std::mem::take(&mut el.overlays).into_iter().enumerate() {
        found.push_back((id, nth, overlay));
    }
    if let Some(tip) = &el.tip {
        tips.push((id, tip.clone()));
        el.sem.description.get_or_insert_with(|| tip.clone());
    }
    let mut seen: Vec<u64> = Vec::with_capacity(el.children.len());
    for (i, child) in el.children.iter_mut().enumerate() {
        // The same identities layout gives.
        let mut cid = child_id(id, child.key.as_deref(), i);
        if seen.contains(&cid) {
            cid = child_id(id, None, i);
        }
        seen.push(cid);
        collect(child, cid, found, tips, relations, ids);
    }
}

/// The root's child that shows overlay `layer` built from `overlay`: its content at the
/// window's top-left (placement moves it), or a dialog's backdrop holding it.
fn entry<S>(id_key: super::super::Key, kind: Kind, content: El<S>) -> (El<S>, u64) {
    let id = child_id(ROOT, Some(&id_key), 0);
    let mut panel = Overlay::defaults(kind, content);
    if kind != Kind::Dialog {
        panel.key = Some(id_key);
        return (panel.absolute(0.0, 0.0), id);
    }
    let panel_id = child_id(id, panel.key.as_deref(), 0);
    let backdrop = super::super::el()
        .key(id_key)
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .align(Align::Center)
        .justify(super::super::Justify::Center)
        .fill(token::SCRIM)
        .transition(Transition::fade())
        .child(panel);
    (backdrop, panel_id)
}

impl<S> Runtime<S> {
    /// After a view run: lifts every overlay out of the new tree and appends the open ones
    /// (nested ones after the overlay holding them, then toasts, then the tooltip) as the
    /// root's last children. An overlay whose trigger went is dismissed.
    pub(super) fn hoist(&mut self) {
        let mut found = VecDeque::new();
        let mut tips = Vec::new();
        let mut relations = HashMap::new();
        let mut ids = (!self.layers.is_empty()).then(HashSet::new);
        collect(&mut self.root, ROOT, &mut found, &mut tips, &mut relations, &mut ids);
        let mut made: Vec<(Layer<S>, El<S>)> = Vec::new();
        while let Some((trigger, nth, overlay)) = found.pop_front() {
            // A trigger says what it opens and whether it is open; toasts are no popup.
            let opens = overlay.kind.popup().is_some();
            if opens {
                let rel = relations.entry(trigger).or_default();
                rel.popup = rel.popup.or(overlay.kind.popup());
                rel.expanded = rel.expanded.or(Some(false));
            }
            if !overlay.open {
                continue;
            }
            let Overlay {
                kind,
                key,
                anchor,
                side,
                align,
                offset,
                match_width,
                focus,
                on_dismiss,
                content,
                ..
            } = overlay;
            let key = key
                .or_else(|| content.key.clone())
                .unwrap_or_else(|| format!("overlay:{trigger:x}:{nth}").into());
            let (mut el, panel) = entry(key.clone(), kind, content);
            let id = child_id(ROOT, Some(&key), 0);
            if opens {
                let rel = relations.entry(trigger).or_default();
                rel.expanded = Some(true);
                rel.controls = Some(hit_of(panel));
            }
            if kind == Kind::Dialog {
                relations.entry(panel).or_default().modal = true;
            }
            collect(&mut el, id, &mut found, &mut tips, &mut relations, &mut ids);
            made.push((
                Layer {
                    id,
                    kind,
                    trigger: Some(trigger),
                    anchor,
                    side,
                    align,
                    offset,
                    match_width,
                    focus,
                    on_dismiss,
                    at: 0,
                    len: 0,
                    escaped: false,
                },
                el,
            ));
        }
        // A tooltip showing stays while its trigger still has one.
        match self.tip.trigger.and_then(|t| tips.iter().find(|(id, _)| *id == t)) {
            Some((trigger, text)) if self.tip.shown => made.push(tip_layer(*trigger, text)),
            _ if self.tip.shown => self.tip = Tip::default(),
            _ => {}
        }
        made.sort_by_key(|(l, _)| l.kind.rank());
        // An overlay whose trigger went with it is dismissed (its state still says open).
        if let Some(ids) = ids {
            for old in &self.layers {
                let gone = !made.iter().any(|(l, _)| l.id == old.id);
                if gone && old.trigger.is_some_and(|t| !ids.contains(&t))
                    && let Some(dismiss) = &old.on_dismiss
                {
                    self.queue.push(Action::Dismiss(dismiss.clone()));
                }
            }
        }
        self.entries = self.root.children.len();
        let (layers, els): (Vec<_>, Vec<_>) = made.into_iter().unzip();
        self.root.children.extend(els);
        self.layers = layers;
        self.tips = tips;
        self.relations = relations;
    }

    /// Opens the tooltip of the trigger it waits on, without running the view.
    pub(super) fn show_tip(&mut self) {
        let Some(trigger) = self.tip.trigger else { return };
        let Some((_, text)) = self.tips.iter().find(|(id, _)| *id == trigger) else {
            return;
        };
        if self.tip.shown || self.layers.iter().any(|l| l.kind == Kind::Tooltip) {
            return;
        }
        let (layer, el) = tip_layer(trigger, text);
        self.root.children.push(el);
        self.layers.push(layer);
        self.tip.shown = true;
        self.stale_layout = true;
    }

    /// Closes the tooltip (it fades out), or stops it opening.
    pub(super) fn hide_tip(&mut self) {
        self.tip.due = None;
        if !std::mem::take(&mut self.tip.shown) {
            return;
        }
        let Some(li) = self.layers.iter().position(|l| l.kind == Kind::Tooltip) else {
            return;
        };
        let layer = self.layers.remove(li);
        let k = self.entries + li;
        if k < self.root.children.len() {
            let el = self.root.children.remove(k);
            if let Some(start) = self.laid.iter().position(|l| l.id == layer.id) {
                self.exit(start, k, el);
            }
        }
        self.stale_layout = true;
    }

    /// Follows the pointer and keyboard focus onto and off tooltip triggers: a rest arms
    /// the delay, leaving closes it. Moving straight from one shown tooltip to another
    /// trigger opens the next at once.
    pub(super) fn follow_tip(&mut self) {
        if self.tips.is_empty() && self.tip.trigger.is_none() {
            return;
        }
        let tip_of = |hit: HitId| self.tips.iter().find(|(id, _)| hit_of(*id) == hit).map(|(id, _)| *id);
        let hovered = self.pointer.and_then(|p| self.list.hit_at(p.at)).and_then(|h| tip_of(h.id));
        let focused = self.ix.focus.filter(|_| self.ix.focus_visible).and_then(tip_of);
        let want = hovered.or(focused);
        if want == self.tip.trigger {
            return;
        }
        let warm = self.tip.shown;
        self.hide_tip();
        self.tip = Tip {
            trigger: want,
            due: want.map(|_| if warm { self.clock } else { self.clock + TOOLTIP_DELAY }),
            ..Tip::default()
        };
    }

    /// Places every open overlay against its anchor in the new layout `laid`: a menu wider
    /// than its content when it follows its anchor's width (one more layout pass), then each
    /// entry's subtree moved where [`place`] says.
    pub(super) fn place_layers(&mut self, mut laid: Vec<Laid>) -> Vec<Laid> {
        let s = self.scale;
        let mut widened = false;
        for li in 0..self.layers.len() {
            let layer = &self.layers[li];
            if !layer.match_width {
                continue;
            }
            let Some(a) = self.anchor_rect(layer, &laid) else { continue };
            let want = a.width() / s;
            let el = &mut self.root.children[self.entries + li];
            if !matches!(el.sizing.min_w, Some(Len::Px(v)) if v == want) {
                el.sizing.min_w = Some(Len::Px(want));
                widened = true;
            }
        }
        if widened {
            laid = self.lay_out();
        }
        let window = Rect::new(0.0, 0.0, self.size.0 as f32, self.size.1 as f32);
        let margin = (8.0 * s).round();
        let inset = |r: Rect| Rect::new(r.left + margin, r.top + margin, r.width() - 2.0 * margin, r.height() - 2.0 * margin);
        let at: HashMap<u64, usize> = laid.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
        let screen = self.screen.filter(|_| self.root.scrolls().is_none());
        let live: HashSet<u64> = self.layers.iter().map(|l| l.id).collect();
        self.orphans.retain(|id| live.contains(id));
        self.anchors.retain(|id, _| live.contains(id));
        for li in 0..self.layers.len() {
            let layer = &self.layers[li];
            let Some(&e) = at.get(&layer.id) else { continue };
            let len = count(&self.root.children[self.entries + li]);
            let anchor = match self.anchor_rect(layer, &laid) {
                Some(a) => {
                    self.anchors.insert(layer.id, a);
                    Some(a)
                }
                None if layer.anchor == Anchor::Window => None,
                // Its anchor went: dismissed once, it stays where it was until it closes.
                None => {
                    if self.orphans.insert(layer.id)
                        && let Some(dismiss) = &layer.on_dismiss
                    {
                        self.queue.push(Action::Dismiss(dismiss.clone()));
                    }
                    Some(self.anchors.get(&layer.id).copied().unwrap_or_default())
                }
            };
            let layer = &mut self.layers[li];
            (layer.at, layer.len, layer.escaped) = (e, len, false);
            if layer.kind == Kind::Dialog {
                continue;
            }
            let r = laid[e].rect;
            let gap = (layer.offset.resolve(&self.theme) * s).round();
            let spot = |bounds: Rect| {
                place(Place {
                    anchor,
                    size: (r.width(), r.height()),
                    bounds,
                    side: layer.side,
                    align: layer.align,
                    gap,
                })
            };
            let inside = if anchor.is_some() { inset(window) } else { window };
            let mut to = spot(inside);
            // Squeezed onto its anchor (or bigger than the window): it goes to the screen.
            if let (Some(screen), true, Some(a)) = (screen, layer.kind.escapes(), anchor) {
                let placed = Rect::new(to.x, to.y, r.width(), r.height());
                let covers = if a.is_empty() {
                    // A point: covered when strictly inside (a menu opens with a corner on it).
                    placed.left < a.left && a.left < placed.right && placed.top < a.top && a.top < placed.bottom
                } else {
                    placed.intersects(&a)
                };
                if covers || r.width() > inside.width() || r.height() > inside.height() {
                    to = spot(inset(screen));
                    let placed = Rect::new(to.x, to.y, r.width(), r.height());
                    layer.escaped = placed.intersect(window) != placed;
                }
            }
            let (dx, dy) = (to.x - r.left, to.y - r.top);
            for l in &mut laid[e..e + len] {
                l.rect = l.rect.offset(dx, dy);
            }
        }
        laid
    }

    /// Where overlay `layer`'s anchor is now in `laid` (its scroll views' resting offsets
    /// taken off); `None` for the window, or an anchor that went.
    fn anchor_rect(&self, layer: &Layer<S>, laid: &[Laid]) -> Option<Rect> {
        let s = self.scale;
        let index = match &layer.anchor {
            Anchor::Window => return None,
            Anchor::Point(x, y) => return Some(Rect::new((x * s).round(), (y * s).round(), 0.0, 0.0)),
            Anchor::Trigger => layer.trigger.and_then(|t| laid.iter().position(|l| l.id == t)),
            Anchor::Key(key) => {
                let mut found = None;
                each(&self.root, laid, &mut |el, i, _| {
                    if found.is_none() && el.key.as_deref() == Some(&**key) {
                        found = Some(i);
                    }
                });
                found
            }
        }?;
        let mut rect = laid[index].rect;
        let mut at = laid[index].parent;
        while let Some(p) = at {
            if let Some(view) = self.scrolls.get(&laid[p].id) {
                let by = view.target();
                rect = match view.axis {
                    Axis::Vertical => rect.offset(0.0, -by),
                    Axis::Horizontal => rect.offset(-by, 0.0),
                };
            }
            at = laid[p].parent;
        }
        Some(rect)
    }

    /// Focus after a relayout: an overlay that takes focus moves it to its selected (or
    /// first) focusable element when it opens, and gives it back when it closes.
    pub(super) fn follow_layers(&mut self) {
        let now: Vec<u64> = self.layers.iter().map(|l| l.id).collect();
        let closed: Vec<u64> = self.shown.iter().copied().filter(|id| !now.contains(id)).collect();
        for id in closed {
            let Some(back) = self.returns.remove(&id) else { continue };
            let present = |hit: HitId| self.laid.iter().any(|l| l.hit == hit);
            if self.ix.focus.is_none_or(|f| !present(f)) {
                self.ix.focus = back.filter(|&b| present(b));
                self.stale_paint = true;
            }
        }
        let opened: Vec<usize> = (0..self.layers.len())
            .filter(|&li| self.layers[li].focus && !self.shown.contains(&self.layers[li].id))
            .collect();
        if !opened.is_empty() {
            let flat = flatten(&self.root);
            for li in opened {
                let layer = &self.layers[li];
                let trigger = layer.trigger.map(hit_of).filter(|&h| {
                    self.laid
                        .iter()
                        .zip(&flat)
                        .any(|(l, el)| l.hit == h && el.interactive() && !el.sem.disabled)
                });
                self.returns.insert(layer.id, self.ix.focus.or(trigger));
                let takes = |&i: &usize| {
                    flat.get(i)
                        .is_some_and(|el| el.interactive() && !el.sem.disabled && el.scrolls().is_none())
                };
                let mut candidates = layer.range().filter(takes);
                let first = candidates.clone().next();
                let chosen = candidates
                    .find(|&i| flat[i].sem.selected == Some(true) || flat[i].sem.checked == Some(true))
                    .or(first);
                if let Some(i) = chosen {
                    self.ix.focus = Some(self.laid[i].hit);
                    self.stale_paint = true;
                }
            }
        }
        self.shown = now;
    }

    /// The topmost open overlay that holds focus in and keeps the pointer from the rest
    /// of the window.
    pub(super) fn trap(&self) -> Option<&Layer<S>> {
        self.layers.iter().rev().find(|l| l.kind.modal())
    }

    /// Where an element is drawn now.
    fn shown_at(&self, index: usize) -> Option<Rect> {
        self.poses.get(index).map(|p| p.rect).or_else(|| self.laid.get(index).map(|l| l.rect))
    }

    fn on_trigger(&self, layer: &Layer<S>, at: Point) -> bool {
        layer
            .trigger
            .and_then(|t| self.laid.iter().position(|l| l.id == t))
            .and_then(|i| self.shown_at(i))
            .is_some_and(|r| r.contains(at))
    }

    /// Pointer input as the overlays see it first; whether it goes no further. A press
    /// outside a popover, menu or dialog (not on its trigger) dismisses it, topmost first,
    /// down to the first modal one, which keeps the press; the pointer reaches nothing outside
    /// an open menu or dialog.
    pub(super) fn overlay_input(&mut self, event: &crate::Event) -> bool {
        use crate::Event;
        match *event {
            Event::Press(p, _) | Event::DoubleClick(p, _) => {
                if self.tip.trigger.is_some() {
                    self.hide_tip();
                    self.tip.quiet = true;
                }
                let mut dismiss = Vec::new();
                let mut kept = false;
                for layer in self.layers.iter().rev().filter(|l| l.kind.light()) {
                    let inside = self.shown_at(layer.panel()).is_some_and(|r| r.contains(p.at));
                    if inside || self.on_trigger(layer, p.at) {
                        break;
                    }
                    dismiss.extend(layer.on_dismiss.clone());
                    if layer.kind.modal() {
                        kept = true;
                        break;
                    }
                }
                self.queue.extend(dismiss.into_iter().map(Action::Dismiss));
                if kept {
                    self.ix.pressed = None;
                }
                kept
            }
            Event::Move(p) | Event::Release(p, _) => {
                let blocked = self.trap().is_some_and(|l| {
                    !self.shown_at(l.panel()).is_some_and(|r| r.contains(p.at)) && !self.on_trigger(l, p.at)
                });
                if blocked && (self.ix.hover.is_some() || self.ix.pressed.is_some()) {
                    self.ix.hover = None;
                    self.ix.pressed = None;
                    self.stale_paint = true;
                }
                blocked
            }
            Event::Deactivated => {
                self.hide_tip();
                let dismiss: Vec<Dismiss<S>> = self
                    .layers
                    .iter()
                    .filter(|l| matches!(l.kind, Kind::Popover | Kind::Menu))
                    .filter_map(|l| l.on_dismiss.clone())
                    .collect();
                self.queue.extend(dismiss.into_iter().map(Action::Dismiss));
                false
            }
            _ => false,
        }
    }

    /// Escape for the overlays, after the focused element and its ancestors: the tooltip
    /// first, then the overlay holding focus (`chain` reached its entry), else the topmost
    /// one that can be dismissed. Whether it was used.
    pub(super) fn escape(&mut self, chain: &[usize]) -> bool {
        let holding = self.layers.iter().rev().find(|l| chain.contains(&l.at) && l.on_dismiss.is_some());
        let topmost = || {
            self.layers
                .iter()
                .rev()
                .find(|l| !matches!(l.kind, Kind::Tooltip | Kind::Toasts) && l.on_dismiss.is_some())
        };
        match holding.or_else(topmost).and_then(|l| l.on_dismiss.clone()) {
            Some(dismiss) => {
                self.queue.push(Action::Dismiss(dismiss));
                true
            }
            None => false,
        }
    }

    /// The Menu key: the context menu of the focused element (or the innermost around it
    /// with one), at its centre.
    pub(super) fn context_key(&mut self, chain: &[usize], focused: Option<usize>) -> bool {
        let flat = flatten(&self.root);
        let Some(&i) = chain.iter().find(|&&i| flat.get(i).is_some_and(|el| el.on_context.is_some())) else {
            return false;
        };
        let c = focused.or(Some(i)).and_then(|f| self.shown_at(f)).unwrap_or_default().center();
        let s = self.scale;
        self.queue.push(Action::Context(self.laid[i].hit, c.x / s, c.y / s));
        true
    }

    /// The secondary button released at `at`: the innermost element under it with a context
    /// handler gets it.
    pub(super) fn context_at(&mut self, at: Point) {
        let flat = flatten(&self.root);
        let n = self.laid.len().min(flat.len()).min(self.poses.len());
        let mut cur = (0..n).rev().find(|&i| self.poses[i].rect.contains(at));
        while let Some(i) = cur {
            if flat[i].on_context.is_some() {
                let s = self.scale;
                self.queue.push(Action::Context(self.laid[i].hit, at.x / s, at.y / s));
                return;
            }
            cur = self.laid[i].parent;
        }
    }

    /// Paint's view of the entries, by their place in `laid`.
    pub(super) fn entries_by_index(&self) -> HashMap<usize, Entry> {
        self.layers
            .iter()
            .map(|l| {
                (
                    l.at,
                    Entry {
                        kind: l.kind,
                        escaped: l.escaped,
                    },
                )
            })
            .collect()
    }

    /// Cuts the escaped entries (`ops` and `hits` ranges of `list`, in order) out of `list`
    /// into popups as large as the entry's laid-out rect (so presence never resizes them)
    /// plus the room its shadow needs; returns what the window shows.
    pub(super) fn cut_popups(&mut self, list: &DisplayList, escaped: &[super::Escaped]) -> Option<DisplayList> {
        self.popups.clear();
        if escaped.is_empty() {
            return None;
        }
        let e = self.theme.elevation;
        let room = ((e.spread + e.drop) * self.scale).ceil();
        let mut window = DisplayList {
            focus: list.focus,
            ..DisplayList::default()
        };
        let (mut op, mut hit) = (0, 0);
        for (id, ops, hits) in escaped {
            let Some(&panel) = self.lifted.get(id) else { continue };
            let rect = Rect {
                left: (panel.left - room).floor(),
                top: (panel.top - room).floor(),
                right: (panel.right + room).ceil(),
                bottom: (panel.bottom + room).ceil(),
            };
            let own = DisplayList {
                ops: list.ops[ops.clone()].to_vec(),
                hits: list.hits[hits.clone()].to_vec(),
                focus: None,
            };
            self.popups.push(Popup {
                id: *id,
                rect,
                list: own.translated(-rect.left, -rect.top),
            });
            window.ops.extend_from_slice(&list.ops[op..ops.start]);
            window.hits.extend_from_slice(&list.hits[hit..hits.start]);
            (op, hit) = (ops.end, hits.end);
        }
        window.ops.extend_from_slice(&list.ops[op..]);
        window.hits.extend_from_slice(&list.hits[hit..]);
        Some(window)
    }

    /// When the runtime next needs a frame though nothing moves (a tooltip's delay, an
    /// [`El::on_timeout`]), on its clock ([`Runtime::now`]); `None` once nothing waits.
    pub fn wake_at(&self) -> Option<Duration> {
        self.tip.due.into_iter().chain(self.timers.values().flatten().copied()).min()
    }

    /// The screen area overlays may use beyond the window (device px from the window's
    /// top-left), when the host can show popup surfaces; `None` keeps them in the window.
    /// Menus, popovers and tooltips that would be squeezed onto their anchor in the window
    /// go there, drawn in [`popups`](Self::popups).
    pub fn set_screen(&mut self, screen: Option<Rect>) {
        if screen != self.screen {
            self.screen = screen;
            if !self.layers.is_empty() {
                self.stale_layout = true;
            }
        }
    }

    /// Overlays drawn outside the window at the latest paint: the host shows each in a popup
    /// surface at its rect, and feeds its input back offset by it. [`list`](Self::list) and
    /// [`snap`](Self::snap) still hold them; [`present`](Self::present) leaves them out.
    pub fn popups(&self) -> &[Popup] {
        &self.popups
    }

    /// Fires the timers that are due on the clock.
    pub(super) fn fire_timers(&mut self) {
        let now = self.clock;
        if self.tip.due.is_some_and(|due| due <= now) {
            self.tip.due = None;
            self.show_tip();
        }
        if self.timers.is_empty() {
            return;
        }
        let due: Vec<u64> = self
            .timers
            .iter()
            .filter(|(_, due)| due.is_some_and(|d| d <= now))
            .map(|(id, _)| *id)
            .collect();
        for id in due {
            self.timers.insert(id, None);
            if let Some(l) = self.laid.iter().find(|l| l.id == id) {
                self.queue.push(Action::Timeout(l.hit));
            }
        }
    }

    /// Arms a timer for every element with a timeout that just appeared; forgets those gone.
    pub(super) fn sync_timers(&mut self) {
        let (timers, clock) = (&mut self.timers, self.clock);
        let mut seen = HashSet::new();
        each(&self.root, &self.laid, &mut |el, i, _| {
            if let Some((after, _)) = &el.on_timeout {
                let id = self.laid[i].id;
                seen.insert(id);
                timers.entry(id).or_insert(Some(clock + *after));
            }
        });
        timers.retain(|id, _| seen.contains(id));
    }

}

/// The Menu (Apps) key.
pub(super) const MENU: KeyCode = KeyCode(0x5D);

/// The tooltip layer and entry for `trigger`.
fn tip_layer<S>(trigger: u64, text: &Arc<str>) -> (Layer<S>, El<S>) {
    let key: super::super::Key = format!("tooltip:{trigger:x}").into();
    let id = child_id(ROOT, Some(&key), 0);
    (
        Layer {
            id,
            kind: Kind::Tooltip,
            trigger: Some(trigger),
            anchor: Anchor::Trigger,
            side: Some(Side::Top),
            align: Align::Center,
            offset: Len::Px(6.0),
            match_width: false,
            focus: false,
            on_dismiss: None,
            at: 0,
            len: 0,
            escaped: false,
        },
        tooltip_el(key, text).absolute(0.0, 0.0),
    )
}
