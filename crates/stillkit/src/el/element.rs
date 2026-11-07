//! Elements: what a view returns. Builders take lengths in design pixels (or theme tokens,
//! [`super::token`]) and colours as [`Rgba`] or tokens; the runtime resolves them against the
//! theme and the scale when it lays the tree out.

use std::sync::Arc;

use accesskit::Role;

use crate::icons::Icon;
use crate::{Frame, HitId, Image, Rect, Rgba, Spring, TextRole, Theme};

/// Names an element among its siblings, across frames: for the keyed diff, hit ids,
/// accessibility, scripts and tests ([`super::Runtime::find`]).
pub type Key = Arc<str>;

/// A length in design pixels, or one read from the theme ([`super::token`]).
#[derive(Clone, Copy)]
pub enum Len {
    Px(f32),
    Theme(fn(&Theme) -> f32),
}

impl Len {
    /// The length in design pixels under `theme`.
    pub fn resolve(self, theme: &Theme) -> f32 {
        match self {
            Self::Px(v) => v,
            Self::Theme(read) => read(theme),
        }
    }
}

impl From<f32> for Len {
    fn from(v: f32) -> Self {
        Self::Px(v)
    }
}

/// A colour, fixed or read from the theme's palette ([`super::token`]).
#[derive(Clone, Copy)]
pub enum Paint {
    Rgba(Rgba),
    Theme(fn(&Theme) -> Rgba),
}

impl Paint {
    pub fn resolve(self, theme: &Theme) -> Rgba {
        match self {
            Self::Rgba(c) => c,
            Self::Theme(read) => read(theme),
        }
    }
}

impl From<Rgba> for Paint {
    fn from(c: Rgba) -> Self {
        Self::Rgba(c)
    }
}

/// The spring an element animates along ([`El::motion`]): fixed, or one of the theme's
/// motion tokens ([`super::token::MOTION_LAYOUT`]...).
#[derive(Clone, Copy)]
pub enum Motion {
    Spring(Spring),
    Theme(fn(&Theme) -> Spring),
}

impl Motion {
    pub fn resolve(self, theme: &Theme) -> Spring {
        match self {
            Self::Spring(s) => s,
            Self::Theme(read) => read(theme),
        }
    }
}

impl From<Spring> for Motion {
    fn from(s: Spring) -> Self {
        Self::Spring(s)
    }
}

/// How an element comes and goes ([`El::transition`]): the pose it enters from and exits
/// to. Every transition fades; `scale` and `slide` also move it (not under reduced motion).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// Opacity when away.
    pub(crate) opacity: f32,
    /// Scale about its centre when away.
    pub(crate) scale: f32,
    /// Offset when away, design px.
    pub(crate) slide: (f32, f32),
}

impl Transition {
    /// Fades in and out.
    pub const fn fade() -> Self {
        Self {
            opacity: 0.0,
            scale: 1.0,
            slide: (0.0, 0.0),
        }
    }

    /// Fades while growing from (and shrinking to) `scale` of its size, about its centre.
    pub const fn scale(scale: f32) -> Self {
        Self {
            scale,
            ..Self::fade()
        }
    }

    /// Fades while sliding in from (and out to) `(x, y)` design px away.
    pub const fn slide(x: f32, y: f32) -> Self {
        Self {
            slide: (x, y),
            ..Self::fade()
        }
    }
}

/// What a handler can ask of the frame its change lands in ([`El::on_press_with`]).
#[derive(Default)]
pub struct Tx {
    pub(crate) instant: bool,
    pub(crate) theme: Option<Theme>,
}

impl Tx {
    /// Applies this change with no motion: everything lands on its target at once.
    pub fn instant(&mut self) {
        self.instant = true;
    }

    /// A new look for the whole view; colours spring across where elements have motion.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
    }
}

/// Where children sit across a row or column (CSS `align-items`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

/// How children share a row's or column's main axis (CSS `justify-content`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    Between,
}

/// A length a box can be given: fixed, or the parent's whole content box.
#[derive(Clone, Copy)]
pub(crate) enum Dim {
    Len(Len),
    Full,
}

/// Draws with the existing kit into the element's laid-out rect; the hit id is the element's
/// own, so a kit control drawn here hovers, presses and focuses like any element.
pub type Painter = Box<dyn Fn(&mut Frame, Rect, HitId)>;

/// How a box places its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Column,
    Row,
    /// Rows of this many equal columns.
    Grid(u16),
}

/// A press handler ([`El::on_press`], [`El::on_press_with`]).
pub(crate) type Press<S> = Box<dyn Fn(&mut S, &mut Tx)>;
/// A drag handler ([`El::on_drag`]): the pointer's place across the box, 0..=1 each way.
pub(crate) type Drag<S> = Box<dyn Fn(&mut S, f32, f32)>;

pub(crate) enum What {
    /// `el()`, `row()`, `col()` and `grid(..)`: a container.
    Box(Flow),
    Text {
        text: Arc<str>,
        role: TextRole,
        /// Wrap into at most this many lines; `None` keeps one line (cut with an ellipsis).
        wrap: Option<usize>,
    },
    Icon(Icon),
    Image(Arc<Image>),
    Paint(Painter),
}

#[derive(Default, Clone, Copy)]
pub(crate) struct Sizing {
    pub w: Option<Dim>,
    pub h: Option<Dim>,
    pub min_w: Option<Len>,
    pub min_h: Option<Len>,
    pub max_w: Option<Len>,
    pub max_h: Option<Len>,
    pub grow: f32,
    pub gap: Option<Len>,
    /// Top, right, bottom, left.
    pub pad: [Option<Len>; 4],
    pub align: Option<Align>,
    pub justify: Option<Justify>,
    pub absolute: Option<(Len, Len)>,
}

#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub fill: Option<Paint>,
    /// Text and icon ink.
    pub color: Option<Paint>,
    pub radius: Option<Len>,
    pub shadow: bool,
    pub opacity: f32,
}

/// How an element animates (`motion` and `instant` reach its whole subtree).
#[derive(Default, Clone, Copy)]
pub(crate) struct Anim {
    pub motion: Option<Motion>,
    pub instant: bool,
    pub transition: Option<Transition>,
}

#[derive(Default, Clone)]
pub(crate) struct Sem {
    pub role: Option<Role>,
    pub label: Option<Arc<str>>,
    pub checked: Option<bool>,
    pub disabled: bool,
    pub focusable: bool,
}

/// One element of a view: a box, text, icon, image or kit painting, with its children.
/// Build with [`el`], [`row`], [`col`], [`text`], [`icon`], [`image`] and [`paint`].
pub struct El<S> {
    pub(crate) what: What,
    pub(crate) key: Option<Key>,
    pub(crate) sizing: Sizing,
    pub(crate) look: Look,
    pub(crate) sem: Sem,
    pub(crate) anim: Anim,
    pub(crate) on_press: Option<Press<S>>,
    pub(crate) on_drag: Option<Drag<S>>,
    pub(crate) children: Vec<El<S>>,
}

fn new<S>(what: What) -> El<S> {
    El {
        what,
        key: None,
        sizing: Sizing::default(),
        look: Look {
            fill: None,
            color: None,
            radius: None,
            shadow: false,
            opacity: 1.0,
        },
        sem: Sem::default(),
        anim: Anim::default(),
        on_press: None,
        on_drag: None,
        children: Vec::new(),
    }
}

/// A box whose children stack top to bottom (the same as [`col`]).
pub fn el<S>() -> El<S> {
    new(What::Box(Flow::Column))
}

/// A box whose children run left to right.
pub fn row<S>() -> El<S> {
    new(What::Box(Flow::Row))
}

/// A box whose children stack top to bottom.
pub fn col<S>() -> El<S> {
    new(What::Box(Flow::Column))
}

/// A grid of `columns` equal columns, filled row by row; [`El::gap`] spaces both ways.
pub fn grid<S>(columns: u16) -> El<S> {
    new(What::Box(Flow::Grid(columns.max(1))))
}

/// One line of text in the theme's [`TextRole::Body`] (see [`El::font`], [`El::wrap`]). It is
/// as tall as the role's line and may shrink, cut with an ellipsis.
pub fn text<S>(text: impl Into<Arc<str>>) -> El<S> {
    new(What::Text {
        text: text.into(),
        role: TextRole::Body,
        wrap: None,
    })
}

/// A vector icon filling a square box, [`super::token::ICON`] across unless sized.
pub fn icon<S>(icon: Icon) -> El<S> {
    new(What::Icon(icon))
}

/// A bitmap, at its own size in device pixels unless sized.
pub fn image<S>(image: Arc<Image>) -> El<S> {
    new(What::Image(image))
}

/// A leaf drawn with the kit into its laid-out rect: `paint(|f, rect, id| f.slider(id, rect,
/// ..))`. Size it (`w`, `h`, `w_full`, `grow`); it has no size of its own.
pub fn paint<S>(painter: impl Fn(&mut Frame, Rect, HitId) + 'static) -> El<S> {
    new(What::Paint(Box::new(painter)))
}

impl<S> El<S> {
    /// Names the element among its siblings. Keyed elements keep their identity when
    /// siblings come and go; give one to everything a script, test or animation refers to.
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn child(mut self, child: El<S>) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = El<S>>) -> Self {
        self.children.extend(children);
        self
    }

    // ---- Size and flex --------------------------------------------------------------------

    pub fn w(mut self, w: impl Into<Len>) -> Self {
        self.sizing.w = Some(Dim::Len(w.into()));
        self
    }

    pub fn h(mut self, h: impl Into<Len>) -> Self {
        self.sizing.h = Some(Dim::Len(h.into()));
        self
    }

    /// Both sides: `w` and `h`.
    pub fn size(self, side: impl Into<Len>) -> Self {
        let side = side.into();
        self.w(side).h(side)
    }

    /// The parent's whole content width.
    pub fn w_full(mut self) -> Self {
        self.sizing.w = Some(Dim::Full);
        self
    }

    /// The parent's whole content height.
    pub fn h_full(mut self) -> Self {
        self.sizing.h = Some(Dim::Full);
        self
    }

    /// Takes this share of the room left on the parent's main axis.
    pub fn grow(mut self, share: f32) -> Self {
        self.sizing.grow = share;
        self
    }

    pub fn min_w(mut self, v: impl Into<Len>) -> Self {
        self.sizing.min_w = Some(v.into());
        self
    }

    pub fn min_h(mut self, v: impl Into<Len>) -> Self {
        self.sizing.min_h = Some(v.into());
        self
    }

    pub fn max_w(mut self, v: impl Into<Len>) -> Self {
        self.sizing.max_w = Some(v.into());
        self
    }

    pub fn max_h(mut self, v: impl Into<Len>) -> Self {
        self.sizing.max_h = Some(v.into());
        self
    }

    /// Space between children.
    pub fn gap(mut self, gap: impl Into<Len>) -> Self {
        self.sizing.gap = Some(gap.into());
        self
    }

    /// Padding on all four sides.
    pub fn pad(mut self, pad: impl Into<Len>) -> Self {
        self.sizing.pad = [Some(pad.into()); 4];
        self
    }

    /// Padding left and right.
    pub fn pad_x(mut self, pad: impl Into<Len>) -> Self {
        let pad = Some(pad.into());
        (self.sizing.pad[1], self.sizing.pad[3]) = (pad, pad);
        self
    }

    /// Padding top and bottom.
    pub fn pad_y(mut self, pad: impl Into<Len>) -> Self {
        let pad = Some(pad.into());
        (self.sizing.pad[0], self.sizing.pad[2]) = (pad, pad);
        self
    }

    /// Where children sit across the main axis.
    pub fn align(mut self, align: Align) -> Self {
        self.sizing.align = Some(align);
        self
    }

    /// How children share the main axis.
    pub fn justify(mut self, justify: Justify) -> Self {
        self.sizing.justify = Some(justify);
        self
    }

    /// Out of the flow, its top-left at `(x, y)` in the parent's box.
    pub fn absolute(mut self, x: impl Into<Len>, y: impl Into<Len>) -> Self {
        self.sizing.absolute = Some((x.into(), y.into()));
        self
    }

    // ---- Style ----------------------------------------------------------------------------

    /// The box's fill, drawn under its children.
    pub fn fill(mut self, fill: impl Into<Paint>) -> Self {
        self.look.fill = Some(fill.into());
        self
    }

    /// The ink of text and icons ([`super::token::LABEL`] by default).
    pub fn color(mut self, color: impl Into<Paint>) -> Self {
        self.look.color = Some(color.into());
        self
    }

    /// Corner radius (clamped to a capsule).
    pub fn radius(mut self, radius: impl Into<Len>) -> Self {
        self.look.radius = Some(radius.into());
        self
    }

    /// The theme's soft elevation shadow under the box.
    pub fn shadow(mut self) -> Self {
        self.look.shadow = true;
        self
    }

    /// Fades the element and everything in it.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.look.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// A text element's role in the theme's type ramp.
    pub fn font(mut self, role: TextRole) -> Self {
        if let What::Text { role: r, .. } = &mut self.what {
            *r = role;
        }
        self
    }

    /// Wraps a text element at word boundaries into at most `lines` lines.
    pub fn wrap(mut self, lines: usize) -> Self {
        if let What::Text { wrap, .. } = &mut self.what {
            *wrap = Some(lines.max(1));
        }
        self
    }

    // ---- Semantics ------------------------------------------------------------------------

    /// The accessibility role. Pressable boxes default to a button (a switch when
    /// [`checked`](Self::checked)), draggable ones to a slider.
    pub fn role(mut self, role: Role) -> Self {
        self.sem.role = Some(role);
        self
    }

    /// What assistive technology reads; the first text inside by default.
    pub fn label(mut self, label: impl Into<Arc<str>>) -> Self {
        self.sem.label = Some(label.into());
        self
    }

    /// For toggles: whether it is on.
    pub fn checked(mut self, on: bool) -> Self {
        self.sem.checked = Some(on);
        self
    }

    /// Announced, but never hovered, focused or pressed.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.sem.disabled = disabled;
        self
    }

    /// Takes keyboard focus even without a handler.
    pub fn focusable(mut self) -> Self {
        self.sem.focusable = true;
        self
    }

    // ---- Motion ---------------------------------------------------------------------------

    /// Animates the element and everything in it along `motion` (a [`Spring`] or a theme
    /// token like [`super::token::MOTION_INTERACTIVE`]) whenever a view run or a new theme
    /// gives it new targets: each edge of its box on its own (a stationary edge never moves),
    /// its fill, text and icon colour (in OKLab, without overshoot; ink leads its fill),
    /// opacity, radius and shadow. Children ride along their parent; a child sets its own
    /// motion to override. Nothing animates on the first frame.
    pub fn motion(mut self, motion: impl Into<Motion>) -> Self {
        self.anim.motion = Some(motion.into());
        self
    }

    /// [`motion`](Self::motion) with the theme's layout spring.
    pub fn animate(self) -> Self {
        self.motion(super::token::MOTION_LAYOUT)
    }

    /// Changes in this subtree land at once, whatever motion an ancestor gives it.
    pub fn instant(mut self) -> Self {
        self.anim.instant = true;
        self
    }

    /// How the element enters when it appears after the first frame, and exits: it stays
    /// drawn, out of hit testing, until its exit settles, and coming back mid-exit reverses
    /// smoothly. It moves along its own motion, or the theme's open spring.
    pub fn transition(mut self, transition: Transition) -> Self {
        self.anim.transition = Some(transition);
        self
    }

    // ---- Handlers -------------------------------------------------------------------------

    /// Runs on `&mut S` between frames after a click (release over the element), Enter or
    /// Space while focused, or assistive technology's invoke. The view then runs again.
    pub fn on_press(mut self, handler: impl Fn(&mut S) + 'static) -> Self {
        self.on_press = Some(Box::new(move |s, _| handler(s)));
        self
    }

    /// [`on_press`](Self::on_press) with the frame's [`Tx`]: `tx.instant()` lands the change
    /// with no motion, `tx.set_theme(..)` switches the look.
    pub fn on_press_with(mut self, handler: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.on_press = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` while the pointer is pressed on the element, with the pointer's place
    /// across its box (`0.0..=1.0` each way): sliders. Its changes land at once (a drag
    /// follows the pointer), with no motion.
    pub fn on_drag(mut self, handler: impl Fn(&mut S, f32, f32) + 'static) -> Self {
        self.on_drag = Some(Box::new(handler));
        self
    }

    /// Whether input lands on it (it takes a hit item that can be enabled).
    pub(crate) fn interactive(&self) -> bool {
        self.on_press.is_some() || self.on_drag.is_some() || self.sem.focusable
    }

    /// Whether it gets a hit item at all: interactive, or carrying semantics of its own.
    pub(crate) fn hit(&self) -> bool {
        self.interactive() || self.sem.role.is_some() || self.sem.label.is_some()
    }

    /// The first text in it, for a default label.
    pub(crate) fn first_text(&self) -> Option<&Arc<str>> {
        match &self.what {
            What::Text { text, .. } => Some(text),
            _ => self.children.iter().find_map(El::first_text),
        }
    }
}
