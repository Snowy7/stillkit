//! Anchored overlays (docs/el.md, "Overlays"): content declared in the view with
//! [`El::overlay`], drawn above the window's content, placed against an anchor and kept inside
//! the window (or, when the host can, the screen). Presence in the view is what open means:
//! `.overlay(menu(items).open(s.open).on_dismiss(|s| s.open = false))`.
//!
//! - [`popover`]: non-modal; it never takes focus unless asked ([`Overlay::take_focus`]).
//! - [`menu`]: takes focus (its first item, or its selected one), arrows move through its
//!   items, Tab stays inside, and the rest of the window takes no pointer input while it is
//!   open.
//! - [`dialog`]: modal, centred over a dimmed backdrop; focus is trapped inside and the
//!   background leaves the accessibility tree.
//! - [`toasts`]: a stack in a corner of the window that never takes focus; time each toast out
//!   with [`El::on_timeout`].
//! - Tooltips are [`El::tooltip`]: the runtime opens them after a hover or keyboard-focus
//!   delay, with no view run.
//!
//! Escape, a press outside and the anchor going away call [`Overlay::on_dismiss`]; focus that
//! an overlay took goes back to where it was (the trigger) when it closes.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use accesskit::{HasPopup, Role};

use super::element::{Align, El, Key, Len, Transition};
use crate::{Point, Rect};

/// The side of its anchor an overlay opens on ([`Overlay::side`]); for overlays placed in the
/// window (a dialog, toasts), the window edge it sits against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    fn vertical(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// What an overlay is: how it opens, takes focus and goes away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Popover,
    Menu,
    Dialog,
    Toasts,
    Tooltip,
}

impl Kind {
    /// Traps focus, and keeps pointer input from the rest of the window.
    pub(crate) fn modal(self) -> bool {
        matches!(self, Self::Menu | Self::Dialog)
    }

    /// Goes away on a press outside it.
    pub(crate) fn light(self) -> bool {
        matches!(self, Self::Popover | Self::Menu | Self::Dialog)
    }

    /// May leave the window for a popup surface when the host can show one.
    pub(crate) fn escapes(self) -> bool {
        matches!(self, Self::Popover | Self::Menu | Self::Tooltip)
    }

    /// Paint order among overlays: toasts over dialogs, tooltips over everything.
    pub(crate) fn rank(self) -> u8 {
        match self {
            Self::Toasts => 1,
            Self::Tooltip => 2,
            _ => 0,
        }
    }

    pub(crate) fn popup(self) -> Option<HasPopup> {
        match self {
            Self::Popover | Self::Dialog => Some(HasPopup::Dialog),
            Self::Menu => Some(HasPopup::Menu),
            Self::Toasts | Self::Tooltip => None,
        }
    }
}

/// What an overlay is placed against.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Anchor {
    /// The element that declared it.
    Trigger,
    /// The first element with this key.
    Key(Key),
    /// A point in the window, design px (a context menu).
    Point(f32, f32),
    /// The window itself.
    Window,
}

/// Runs when an overlay should close ([`Overlay::on_dismiss`]).
pub(crate) type Dismiss<S> = Rc<dyn Fn(&mut S)>;

/// Content shown above the window's content, placed against an anchor. Build with
/// [`popover`], [`menu`], [`dialog`] or [`toasts`], and declare it on its trigger with
/// [`El::overlay`].
pub struct Overlay<S> {
    pub(crate) kind: Kind,
    pub(crate) open: bool,
    pub(crate) key: Option<Key>,
    pub(crate) anchor: Anchor,
    pub(crate) side: Option<Side>,
    pub(crate) align: Align,
    pub(crate) offset: Len,
    pub(crate) match_width: bool,
    pub(crate) focus: bool,
    pub(crate) on_dismiss: Option<Dismiss<S>>,
    pub(crate) content: El<S>,
}

fn new<S>(kind: Kind, content: El<S>) -> Overlay<S> {
    let (anchor, side, align, offset, focus) = match kind {
        Kind::Popover => (Anchor::Trigger, Some(Side::Bottom), Align::Center, 6.0, false),
        Kind::Menu => (Anchor::Trigger, Some(Side::Bottom), Align::Start, 4.0, true),
        Kind::Dialog => (Anchor::Window, None, Align::Center, 0.0, true),
        Kind::Toasts => (Anchor::Window, Some(Side::Bottom), Align::End, 16.0, false),
        Kind::Tooltip => (Anchor::Trigger, Some(Side::Top), Align::Center, 6.0, false),
    };
    Overlay {
        kind,
        open: true,
        key: None,
        anchor,
        side,
        align,
        offset: Len::Px(offset),
        match_width: false,
        focus,
        on_dismiss: None,
        content,
    }
}

/// A non-modal panel under its trigger (a picker, a share sheet). It takes no focus unless
/// [`take_focus`](Overlay::take_focus); a press outside, Escape or the trigger going away
/// dismisses it, and the press still lands where it was.
pub fn popover<S>(content: El<S>) -> Overlay<S> {
    new(Kind::Popover, content)
}

/// A menu below its trigger's left edge (or at a point: [`Overlay::at`]). It takes focus on its
/// selected item (else its first), arrows move through its items, Tab stays inside, and while
/// it is open the rest of the window takes no pointer input: a press outside only dismisses.
pub fn menu<S>(content: El<S>) -> Overlay<S> {
    new(Kind::Menu, content)
}

/// A modal panel centred in the window over a dimmed backdrop. Focus moves inside and is
/// trapped there, the background leaves the accessibility tree, and a press on the backdrop
/// or Escape dismisses it.
pub fn dialog<S>(content: El<S>) -> Overlay<S> {
    new(Kind::Dialog, content)
}

/// A stack of toasts against the window's bottom-right corner (move it with
/// [`side`](Overlay::side) and [`align`](Overlay::align)). It never takes focus or input
/// beyond its own; give each toast a transition and [`El::on_timeout`].
pub fn toasts<S>(stack: El<S>) -> Overlay<S> {
    new(Kind::Toasts, stack)
}

impl<S> Overlay<S> {
    /// Whether it shows: `menu(..).open(s.menu)`. A closed overlay still tells assistive
    /// technology that its trigger opens one (collapsed).
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Its identity across frames (and for [`super::Runtime::find`]); by default its
    /// content's key, else its trigger's identity.
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Placed against the first element keyed `key` instead of its trigger.
    pub fn anchor(mut self, key: impl Into<Key>) -> Self {
        self.anchor = Anchor::Key(key.into());
        self
    }

    /// Placed at a point in the window (design px): a context menu at the pointer
    /// ([`El::on_context_menu`]). It opens down and right, flipping where it would not fit.
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.anchor = Anchor::Point(x, y);
        self.offset = Len::Px(0.0);
        self
    }

    /// The anchor's side it prefers; it flips to the other one where it would not fit.
    pub fn side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Where it sits along the anchor's side: `Start` lines up the leading edges, `End` the
    /// trailing ones; it flips to the other edge, then shifts, to stay inside.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// The gap between it and its anchor (or the window edge), design px or a token.
    pub fn offset(mut self, offset: impl Into<Len>) -> Self {
        self.offset = offset.into();
        self
    }

    /// At least as wide as its anchor (a select's menu under its field).
    pub fn match_width(mut self) -> Self {
        self.match_width = true;
        self
    }

    /// A popover takes focus when it opens, like a menu (and gives it back when it closes).
    pub fn take_focus(mut self) -> Self {
        self.focus = true;
        self
    }

    /// Runs when it should close: Escape, a press outside it (not on its trigger), the
    /// window losing activation (popovers and menus), or its anchor going away. Set the
    /// state that opened it back: `on_dismiss(|s| s.open = false)`.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut S) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    /// Its accessibility role and look by default, as the root element of an overlay entry.
    pub(crate) fn defaults(kind: Kind, mut el: El<S>) -> El<S> {
        match kind {
            Kind::Popover => {
                el.sem.role.get_or_insert(Role::Dialog);
                el.anim.transition.get_or_insert(Transition::scale(0.96));
            }
            Kind::Menu => {
                el.sem.role.get_or_insert(Role::Menu);
                el.anim.transition.get_or_insert(Transition::scale(0.96));
                if el.group.is_none() {
                    el = el.focus_group(super::Axis::Vertical);
                }
            }
            Kind::Dialog => {
                el.sem.role.get_or_insert(Role::Dialog);
            }
            Kind::Toasts | Kind::Tooltip => {}
        }
        el
    }
}

/// How long the pointer (or keyboard focus) rests on a trigger before its tooltip opens.
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

/// A tooltip's panel: its text on the theme's elevated fill.
pub(crate) fn tooltip_el<S>(key: Key, text: &Arc<str>) -> El<S> {
    use super::{text as label, token};
    super::el()
        .key(key)
        .pad_x(8.0)
        .pad_y(4.0)
        .max_w(280.0)
        .radius(token::RADIUS_SMALL)
        .fill(token::ELEVATED)
        .shadow()
        .transition(Transition::fade())
        .child(label(text.clone()).font(crate::TextRole::Footnote).wrap(4))
}

/// What a placement needs (device px).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Place {
    /// The anchor's rect; `None` places against `bounds` itself.
    pub anchor: Option<Rect>,
    pub size: (f32, f32),
    /// Where it must stay.
    pub bounds: Rect,
    pub side: Option<Side>,
    pub align: Align,
    pub gap: f32,
}

/// The top-left an overlay of `p.size` goes to, on whole pixels. Against an anchor: on the
/// preferred side, else the opposite one if only that fits (else the roomier, clamped); along
/// the side by its alignment, flipped to the other edge when it would run out, then shifted
/// inside `bounds`. Against the bounds: at the side's edge (centred with no side), by its
/// alignment along it.
pub(crate) fn place(p: Place) -> Point {
    let (w, h) = p.size;
    let b = p.bounds;
    // `start..end` along one axis, keeping `len` inside.
    let clamp = |v: f32, len: f32, start: f32, end: f32| v.min(end - len).max(start);
    let (x, y) = match p.anchor {
        None => {
            let along = |start: f32, end: f32, len: f32| match p.align {
                Align::Start | Align::Stretch => start + p.gap,
                Align::Center => (start + end - len) / 2.0,
                Align::End => end - p.gap - len,
            };
            match p.side {
                None => ((b.left + b.right - w) / 2.0, (b.top + b.bottom - h) / 2.0),
                Some(Side::Top) => (along(b.left, b.right, w), b.top + p.gap),
                Some(Side::Bottom) => (along(b.left, b.right, w), b.bottom - p.gap - h),
                Some(Side::Left) => (b.left + p.gap, along(b.top, b.bottom, h)),
                Some(Side::Right) => (b.right - p.gap - w, along(b.top, b.bottom, h)),
            }
        }
        Some(a) => {
            let side = p.side.unwrap_or(Side::Bottom);
            // Main axis: (anchor start, anchor end, own length, bounds start, bounds end).
            let (a0, a1, len, b0, b1) = if side.vertical() {
                (a.top, a.bottom, h, b.top, b.bottom)
            } else {
                (a.left, a.right, w, b.left, b.right)
            };
            let at = |s: Side| match s {
                Side::Bottom | Side::Right => a1 + p.gap,
                Side::Top | Side::Left => a0 - p.gap - len,
            };
            let fits = |v: f32| v >= b0 && v + len <= b1;
            let room = |s: Side| match s {
                Side::Bottom | Side::Right => b1 - a1,
                Side::Top | Side::Left => a0 - b0,
            };
            let main = if fits(at(side)) {
                at(side)
            } else if fits(at(side.opposite())) || room(side.opposite()) > room(side) {
                at(side.opposite())
            } else {
                at(side)
            };
            let main = clamp(main, len, b0, b1);
            // Cross axis.
            let (c0, c1, cross, d0, d1) = if side.vertical() {
                (a.left, a.right, w, b.left, b.right)
            } else {
                (a.top, a.bottom, h, b.top, b.bottom)
            };
            let start = c0;
            let end = c1 - cross;
            let cross_at = match p.align {
                Align::Start | Align::Stretch if start + cross > d1 && end >= d0 => end,
                Align::Start | Align::Stretch => start,
                Align::End if end < d0 && start + cross <= d1 => start,
                Align::End => end,
                Align::Center => (c0 + c1 - cross) / 2.0,
            };
            let cross_at = clamp(cross_at, cross, d0, d1);
            if side.vertical() { (cross_at, main) } else { (main, cross_at) }
        }
    };
    Point::new(x.round(), y.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(anchor: Option<Rect>, side: Option<Side>, align: Align) -> Point {
        place(Place {
            anchor,
            size: (100.0, 50.0),
            bounds: Rect::new(0.0, 0.0, 400.0, 300.0),
            side,
            align,
            gap: 4.0,
        })
    }

    #[test]
    fn it_opens_on_its_side_and_flips_where_it_would_not_fit() {
        let a = Rect::new(150.0, 100.0, 60.0, 20.0);
        assert_eq!(at(Some(a), Some(Side::Bottom), Align::Start), Point::new(150.0, 124.0));
        assert_eq!(at(Some(a), Some(Side::Top), Align::End), Point::new(110.0, 46.0));
        assert_eq!(at(Some(a), Some(Side::Right), Align::Center), Point::new(214.0, 85.0));
        // Near the bottom it opens upward; near the right edge its left edge flips to the
        // anchor's right edge.
        let low = Rect::new(330.0, 260.0, 60.0, 20.0);
        assert_eq!(at(Some(low), Some(Side::Bottom), Align::Start), Point::new(290.0, 206.0));
        // A point (a context menu) near the bottom-right corner opens up and to the left.
        let p = Rect::new(390.0, 290.0, 0.0, 0.0);
        assert_eq!(at(Some(p), Some(Side::Bottom), Align::Start), Point::new(290.0, 236.0));
    }

    #[test]
    fn it_shifts_inside_and_window_placements_use_the_edges() {
        // Centred over a narrow anchor at the left edge: shifted in.
        let a = Rect::new(0.0, 100.0, 20.0, 20.0);
        assert_eq!(at(Some(a), Some(Side::Bottom), Align::Center), Point::new(0.0, 124.0));
        assert_eq!(at(None, None, Align::Center), Point::new(150.0, 125.0));
        assert_eq!(at(None, Some(Side::Bottom), Align::End), Point::new(296.0, 246.0));
    }
}
