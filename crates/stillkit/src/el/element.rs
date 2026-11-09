//! Elements: what a view returns. Builders take lengths in design pixels (or theme tokens,
//! [`super::token`]) and colours as [`Rgba`] or tokens; the runtime resolves them against the
//! theme and the scale when it lays the tree out.

use std::sync::Arc;
use std::time::Duration;

use accesskit::{HasPopup, Role};

use crate::icons::Icon;
use crate::{Frame, HitId, Image, Rect, Rgba, Spring, TextRole, Theme};

/// Names an element among its siblings, across frames: for the keyed diff, hit ids,
/// accessibility, scripts and tests ([`super::Runtime::find`]).
pub type Key = Arc<str>;

/// A length in design pixels, or one read from the theme ([`super::token`]).
#[derive(Clone, Copy)]
pub enum Len {
    /// Px.
    Px(f32),
    /// Theme.
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
    /// Rgba.
    Rgba(Rgba),
    /// Theme.
    Theme(fn(&Theme) -> Rgba),
}

impl Paint {
    /// Resolve.
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
    /// Spring.
    Spring(Spring),
    /// Theme.
    Theme(fn(&Theme) -> Spring),
}

impl Motion {
    /// Resolve.
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

/// What a handler can ask of the frame its change lands in ([`El::on_press_with`],
/// [`El::on_key`]).
#[derive(Default)]
pub struct Tx {
    pub(crate) instant: bool,
    pub(crate) mods: Modifiers,
    pub(crate) theme: Option<Theme>,
    pub(crate) reveal: Vec<Key>,
    /// Where focus goes once the view has run: an element by key, or nowhere (`None`).
    pub(crate) focus: Option<Option<Key>>,
}

impl Tx {
    /// Applies this change with no motion: everything lands on its target at once.
    pub fn instant(&mut self) {
        self.instant = true;
    }

    /// The modifier keys held with the key an [`El::on_key`] handler is looking at (none for a
    /// press or a timer).
    pub fn modifiers(&self) -> Modifiers {
        self.mods
    }

    /// A new look for the whole view; colours spring across where elements have motion.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
    }

    /// Once the view has run, scrolls every scroll view around the element keyed `key` just
    /// enough to show it (a row just added, a search hit).
    pub fn scroll_into_view(&mut self, key: impl Into<Key>) {
        self.reveal.push(key.into());
    }

    /// Once the view has run, moves keyboard focus to the first element keyed `key` (the
    /// field a shortcut opens, the parent a tree's Left goes to) and scrolls it into view. Its
    /// ring shows when a key brought focus there (an [`El::on_key`] handler), or the ring was
    /// already showing. A key no focusable element has leaves focus where it is.
    pub fn focus(&mut self, key: impl Into<Key>) {
        self.focus = Some(Some(key.into()));
    }

    /// Once the view has run, takes keyboard focus away (a search field's Escape): nothing
    /// has it, and its ring goes.
    pub fn blur(&mut self) {
        self.focus = Some(None);
    }
}

/// A key going down, as [`El::on_key`] sees it: a virtual-key code (`Event::Key`), with names
/// for the common ones. Match on the names: `KeyCode::ENTER => ..`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCode(pub u16);

impl KeyCode {
    /// Backspace.
    pub const BACKSPACE: Self = Self(0x08);
    /// Tab.
    pub const TAB: Self = Self(0x09);
    /// Enter.
    pub const ENTER: Self = Self(0x0D);
    /// Escape.
    pub const ESCAPE: Self = Self(0x1B);
    /// Space.
    pub const SPACE: Self = Self(0x20);
    /// Page up.
    pub const PAGE_UP: Self = Self(0x21);
    /// Page down.
    pub const PAGE_DOWN: Self = Self(0x22);
    /// End.
    pub const END: Self = Self(0x23);
    /// Home.
    pub const HOME: Self = Self(0x24);
    /// Left.
    pub const LEFT: Self = Self(0x25);
    /// Up.
    pub const UP: Self = Self(0x26);
    /// Right.
    pub const RIGHT: Self = Self(0x27);
    /// Down.
    pub const DOWN: Self = Self(0x28);
    /// Insert.
    pub const INSERT: Self = Self(0x2D);
    /// Delete.
    pub const DELETE: Self = Self(0x2E);

    /// A letter or digit key (`KeyCode::of('A')`): its virtual-key code is the upper-case
    /// ASCII character.
    pub const fn of(c: char) -> Self {
        Self(c.to_ascii_uppercase() as u16)
    }
}

/// The modifier keys held with a key ([`Tx::modifiers`], [`super::Runtime::key_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Ctrl.
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// The Windows or Super key.
    pub meta: bool,
}

impl Modifiers {
    /// None.
    pub const NONE: Self = Self { shift: false, ctrl: false, alt: false, meta: false };
    /// Shift.
    pub const SHIFT: Self = Self { shift: true, ..Self::NONE };
    /// Ctrl.
    pub const CTRL: Self = Self { ctrl: true, ..Self::NONE };
    /// Alt.
    pub const ALT: Self = Self { alt: true, ..Self::NONE };
}

/// Both sets held: `Modifiers::CTRL | Modifiers::SHIFT`.
impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, o: Self) -> Self {
        Self {
            shift: self.shift || o.shift,
            ctrl: self.ctrl || o.ctrl,
            alt: self.alt || o.alt,
            meta: self.meta || o.meta,
        }
    }
}

/// Whether a key handler used the key ([`El::on_key`]); an unused key goes on to the
/// element's built-in keys (a range, a group, a scroll view) and then its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// Yes.
    Yes,
    /// No.
    No,
}

impl From<bool> for Handled {
    fn from(used: bool) -> Self {
        if used { Self::Yes } else { Self::No }
    }
}

/// A direction: a scroll view's ([`El::scroll`]) or a focus group's ([`El::focus_group`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Horizontal.
    Horizontal,
    /// Vertical.
    Vertical,
}

/// The arrows a focus group moves along ([`El::focus_group`]).
pub type Orientation = Axis;

/// Where an arc starts ([`arc`], [`El::arc_start`]); it runs clockwise from there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArcStart {
    /// Twelve o'clock: a ring.
    #[default]
    Top,
    /// Three o'clock.
    Right,
    /// Six o'clock.
    Bottom,
    /// Nine o'clock: with half a turn of span, a semicircle meter.
    Left,
}

impl ArcStart {
    /// Radians clockwise from twelve o'clock.
    pub(crate) fn angle(self) -> f32 {
        let quarters = match self {
            Self::Top => 0.0,
            Self::Right => 1.0,
            Self::Bottom => 2.0,
            Self::Left => 3.0,
        };
        quarters * std::f32::consts::FRAC_PI_2
    }
}

/// Where children sit across a row or column (CSS `align-items`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Start.
    Start,
    /// Center.
    Center,
    /// End.
    End,
    /// Stretch.
    Stretch,
}

/// How children share a row's or column's main axis (CSS `justify-content`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    /// Start.
    Start,
    /// Center.
    Center,
    /// End.
    End,
    /// Between.
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
/// A key handler ([`El::on_key`]).
pub(crate) type OnKey<S> = Box<dyn Fn(&mut S, KeyCode, &mut Tx) -> Handled>;
/// A range's step handler ([`El::on_step`]): the change in value.
pub(crate) type OnStep<S> = Box<dyn Fn(&mut S, f64)>;
/// A character handler ([`El::on_char`]).
pub(crate) type OnChar<S> = Box<dyn Fn(&mut S, char, &mut Tx) -> Handled>;
/// A text field's handler ([`El::on_input`], [`El::on_submit`]): the field's text.
pub(crate) type OnText<S> = Box<dyn Fn(&mut S, &str)>;
/// A context-menu handler ([`El::on_context_menu`]): where, in design px.
pub(crate) type OnContext<S> = Box<dyn Fn(&mut S, f32, f32)>;
/// A timeout handler ([`El::on_timeout`]).
pub(crate) type OnTimeout<S> = (Duration, Box<dyn Fn(&mut S)>);
/// A pointer handler ([`El::on_pointer`]): whether it changed the state (the view runs).
pub(crate) type OnPointer<S> = Box<dyn Fn(&mut S, &PointerEvent, &mut Tx) -> bool>;
/// A file-drop handler ([`El::on_drop_files`]): the dropped paths.
pub(crate) type OnFiles<S> = Box<dyn Fn(&mut S, &[std::path::PathBuf])>;
/// A wheel handler ([`El::on_wheel`]): whether it changed the state (the view runs).
pub(crate) type OnWheel<S> = Box<dyn Fn(&mut S, &WheelEvent, &mut Tx) -> bool>;

/// What a pointer did on an element with [`El::on_pointer`]. Positions are design px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    /// What happened.
    pub phase: PointerPhase,
    /// The button pressed (for every phase of a press; `Left` while hovering).
    pub button: crate::Button,
    /// Where the pointer is, from the window's top-left.
    pub at: crate::Point,
    /// Where it is from the element's top-left (outside its box during a captured drag).
    pub local: crate::Point,
    /// How far it moved since the element's last event (moves within a frame add up).
    pub delta: crate::Point,
    /// Where the press began, from the window's top-left (`at` while hovering).
    pub origin: crate::Point,
    /// The element's size.
    pub size: (f32, f32),
    /// The modifier keys held.
    pub mods: Modifiers,
    /// 1 for a click, 2 for a double click (on `Down`).
    pub clicks: u8,
}

impl PointerEvent {
    /// How far the pointer is from where the press began.
    pub fn travel(&self) -> crate::Point {
        crate::Point::new(self.at.x - self.origin.x, self.at.y - self.origin.y)
    }
}

/// The phase of a [`PointerEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
    /// It moved over the element with no button held.
    Hover,
    /// A button went down on the element: the element holds the pointer until it is released.
    Down,
    /// It moved while held (anywhere, even outside the element or the window).
    Move,
    /// The button went up.
    Up,
    /// The press ended without a release (the window lost the pointer or deactivated).
    Cancel,
}

/// The wheel turned over an element with [`El::on_wheel`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelEvent {
    /// Where the pointer is, from the window's top-left (design px).
    pub at: crate::Point,
    /// Where it is from the element's top-left (design px).
    pub local: crate::Point,
    /// Notches, positive away from the user (up); a touchpad gives fractions.
    pub delta: f32,
    /// The modifier keys held.
    pub mods: Modifiers,
}

/// A range's values ([`El::range`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Range {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
}

/// An arc's look ([`arc`]).
#[derive(Clone, Copy)]
pub(crate) struct ArcLook {
    /// How much of the span is filled, 0..=1.
    pub fraction: f32,
    /// Radians clockwise from twelve o'clock.
    pub start: f32,
    /// Of a whole turn, 0..=1.
    pub span: f32,
    pub stroke: Len,
    /// The unfilled rest of the span; `None` draws no track.
    pub track: Option<Paint>,
}

/// A text field's options ([`text_input`], [`text_area`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldSpec {
    pub multiline: bool,
    /// The text the view gives it (`None`: the field keeps its own).
    pub value: Option<Arc<str>>,
    pub placeholder: Option<Arc<str>>,
    pub secure: bool,
    pub max_len: Option<usize>,
    pub read_only: bool,
    pub select_all: bool,
    pub face: Face,
    /// A text area's height in lines when it is given none.
    pub rows: usize,
}

/// Where a line of text sits across its box ([`El::text_align`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// The left edge (the default).
    #[default]
    Start,
    /// Centred.
    Center,
    /// The right edge: numbers in a column.
    End,
}

/// How text is set: a theme role ([`El::font`]), with optionally its own family, weight and
/// size ([`El::family`], [`El::weight`], [`El::text_size`]) and alignment.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub(crate) role: TextRole,
    pub(crate) family: Option<Arc<str>>,
    pub(crate) weight: Option<u16>,
    /// Design px.
    pub(crate) size: Option<f32>,
    pub(crate) align: TextAlign,
}

impl Default for Face {
    fn default() -> Self {
        Self {
            role: TextRole::Body,
            family: None,
            weight: None,
            size: None,
            align: TextAlign::Start,
        }
    }
}

impl Face {
    /// The text style under `theme` at `s` device px per design px: the role's, with this
    /// face's overrides.
    pub(crate) fn style(&self, theme: &Theme, s: f32) -> crate::TextStyle {
        let mut style = theme.font.style(self.role, s);
        if let Some(family) = &self.family {
            style.family = family.clone();
        }
        if let Some(weight) = self.weight {
            style.weight = weight;
        }
        if let Some(size) = self.size {
            let role = theme.font.font(self.role).size.max(1.0);
            style.tracking *= size / role;
            style.size = size * s;
            style.optical = size * 0.75;
        }
        style
    }

    /// The baseline-to-baseline distance in device px: the role's, scaled with the size.
    pub(crate) fn line(&self, theme: &Theme, s: f32) -> f32 {
        let font = theme.font.font(self.role);
        match self.size {
            Some(size) => (font.line * size / font.size.max(1.0) * s).round(),
            None => theme.font.line(self.role, s),
        }
    }
}

pub(crate) enum What {
    /// `el()`, `row()`, `col()` and `grid(..)`: a container.
    Box(Flow),
    /// [`text_input`] and [`text_area`].
    Field(FieldSpec),
    Text {
        text: Arc<str>,
        face: Face,
        /// Wrap into at most this many lines; `None` keeps one line (cut with an ellipsis).
        wrap: Option<usize>,
    },
    Icon(Icon),
    Image(Arc<Image>),
    Paint(Painter),
    Arc(ArcLook),
    /// [`native`]: a hole showing the host's native layer of this name.
    Native(Key),
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
    /// Children flow onto further lines when they overflow the main axis.
    pub flow_wrap: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Look {
    pub fill: Option<Paint>,
    /// Text and icon ink.
    pub color: Option<Paint>,
    pub radius: Option<Len>,
    pub shadow: Option<Cast>,
    pub opacity: f32,
    /// A stroke inside the box's edge ([`El::border`]).
    pub border: Option<Border>,
}

impl Look {
    /// This look with what `patch` sets (a state style: [`El::hovered`], [`El::pressed`],
    /// [`El::focus_within`]) laid over it.
    pub fn with(self, patch: Self) -> Self {
        Self {
            fill: patch.fill.or(self.fill),
            color: patch.color.or(self.color),
            radius: patch.radius.or(self.radius),
            shadow: patch.shadow.or(self.shadow),
            opacity: if patch.opacity < 1.0 { patch.opacity } else { self.opacity },
            border: patch.border.or(self.border),
        }
    }
}

/// A box's border: a stroke of `width` design px inside its edge, following its radius.
#[derive(Clone, Copy)]
pub(crate) struct Border {
    pub width: Len,
    pub paint: Paint,
}

/// The looks an element takes in interaction states, laid over its own in this order
/// (pressed wins): pointer over it or inside it, focus on it or inside it, pressed on it.
#[derive(Clone, Copy, Default)]
pub(crate) struct States {
    pub hover: Option<Look>,
    pub within: Option<Look>,
    pub press: Option<Look>,
    /// While dragged files are over it ([`El::dropping`]).
    pub drop: Option<Look>,
}

impl States {
    pub fn any(&self) -> bool {
        self.hover.is_some() || self.within.is_some() || self.press.is_some() || self.drop.is_some()
    }
}

/// Which elements are in an interaction state now (identities), for [`States`].
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Active {
    pub hover: Vec<u64>,
    pub within: Vec<u64>,
    pub press: Vec<u64>,
    pub drop: Vec<u64>,
}

impl Active {
    /// `look` with the state styles of element `id` laid over it.
    pub fn look(&self, id: u64, look: Look, states: &States) -> Look {
        let mut out = look;
        for (on, patch) in [(&self.hover, states.hover), (&self.within, states.within), (&self.press, states.press), (&self.drop, states.drop)] {
            if let Some(patch) = patch.filter(|_| on.contains(&id)) {
                out = out.with(patch);
            }
        }
        out
    }
}

/// The shadow a box casts: one of the theme's levels, or a custom one.
#[derive(Clone, Copy)]
pub(crate) enum Cast {
    Level(crate::ShadowLevel),
    Custom(crate::SoftShadow),
}

impl Cast {
    /// Its layers in design px under `theme` (a custom shadow's second is transparent).
    pub fn layers(self, theme: &Theme) -> [crate::SoftShadow; 2] {
        match self {
            Self::Level(level) => theme.shadow(level),
            Self::Custom(one) => [one, crate::SoftShadow { color: Rgba::TRANSPARENT, ..one }],
        }
    }
}

/// How an element animates (`motion` and `instant` reach its whole subtree).
#[derive(Default, Clone, Copy)]
pub(crate) struct Anim {
    pub motion: Option<Motion>,
    pub instant: bool,
    pub transition: Option<Transition>,
    /// Radians clockwise about its centre, at draw time ([`El::rotate`]).
    pub rotate: f32,
    /// One turn every this long while it is visible ([`El::spin`]).
    pub spin: Option<Duration>,
}

#[derive(Default, Clone)]
pub(crate) struct Sem {
    pub role: Option<Role>,
    pub label: Option<Arc<str>>,
    pub checked: Option<bool>,
    pub disabled: bool,
    pub focusable: bool,
    pub range: Option<Range>,
    pub description: Option<Arc<str>>,
    /// The value read as text ("40%", "Medium").
    pub value: Option<Arc<str>>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    /// Its value fails validation ([`El::invalid`]).
    pub invalid: bool,
    /// The key of the descendant it shows as active ([`El::active_descendant`]).
    pub active: Option<Key>,
}

/// A roving focus group ([`El::focus_group`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Group {
    pub axis: Axis,
    /// Arrows also press the member they land on (radio groups, segmented controls).
    pub follow: bool,
}

/// One element of a view: a box, text, icon, image, arc or kit painting, with its children.
/// Build with [`el`], [`row`], [`col`], [`text`], [`icon`], [`image`], [`arc`] and [`paint`].
pub struct El<S> {
    pub(crate) what: What,
    pub(crate) key: Option<Key>,
    pub(crate) sizing: Sizing,
    pub(crate) look: Look,
    pub(crate) sem: Sem,
    pub(crate) anim: Anim,
    pub(crate) on_press: Option<Press<S>>,
    pub(crate) on_drag: Option<Drag<S>>,
    pub(crate) on_pointer: Option<OnPointer<S>>,
    pub(crate) on_wheel: Option<OnWheel<S>>,
    /// Files dropped on it ([`El::on_drop_files`]).
    pub(crate) on_files: Option<OnFiles<S>>,
    pub(crate) on_key: Option<OnKey<S>>,
    pub(crate) on_step: Option<OnStep<S>>,
    pub(crate) group: Option<Group>,
    /// A scroll view along this axis ([`El::scroll`]); boxes only.
    pub(crate) scroll: Option<Axis>,
    /// A scroll view that stays at its end while content grows ([`El::follow_end`]).
    pub(crate) follow_end: bool,
    pub(crate) on_context: Option<OnContext<S>>,
    pub(crate) on_timeout: Option<OnTimeout<S>>,
    pub(crate) on_char: Option<OnChar<S>>,
    pub(crate) on_input: Option<OnText<S>>,
    pub(crate) on_submit: Option<OnText<S>>,
    /// Overlays declared here ([`El::overlay`]); the runtime lifts them above the content
    /// after each view run.
    pub(crate) overlays: Vec<super::Overlay<S>>,
    /// What it opens, for assistive technology, even while closed.
    pub(crate) popup: Option<HasPopup>,
    /// Its tooltip's text ([`El::tooltip`]).
    pub(crate) tip: Option<Arc<str>>,
    /// The looks it takes while hovered, focused within or pressed ([`El::hovered`]...).
    pub(crate) states: States,
    /// The pointer's shape over it ([`El::cursor`]).
    pub(crate) cursor: Option<crate::Cursor>,
    /// Its ink flows to descendants that set none ([`El::cascade_ink`]).
    pub(crate) cascade: bool,
    /// What a [`virtual_list`](super::virtual_list) built.
    pub(crate) virt: Option<super::virtualize::Virt>,
    /// Draws no focus ring of its own ([`El::focus_ring`]).
    pub(crate) quiet: bool,
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
            shadow: None,
            opacity: 1.0,
            border: None,
        },
        sem: Sem::default(),
        anim: Anim::default(),
        on_press: None,
        on_drag: None,
        on_pointer: None,
        on_wheel: None,
        on_files: None,
        on_key: None,
        on_step: None,
        group: None,
        scroll: None,
        follow_end: false,
        on_context: None,
        on_timeout: None,
        on_char: None,
        on_input: None,
        on_submit: None,
        overlays: Vec::new(),
        popup: None,
        tip: None,
        states: States::default(),
        cursor: None,
        virt: None,
        cascade: false,
        quiet: false,
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
        face: Face::default(),
        wrap: None,
    })
}

fn field<S>(key: Key, multiline: bool) -> El<S> {
    let mut el = new(What::Field(FieldSpec {
        multiline,
        value: None,
        placeholder: None,
        secure: false,
        max_len: None,
        read_only: false,
        select_all: false,
        face: Face::default(),
        rows: 3,
    }));
    el.key = Some(key);
    el
}

/// A one-line text field named `key`: caret, selection, mouse and keyboard editing, undo,
/// the clipboard and IME. Give it its text with [`El::value`] and take edits with
/// [`El::on_input`] (the view owns the string: a value the view doesn't take back stays as
/// it was); Enter runs [`El::on_submit`]. It is one line of its type role tall plus its
/// padding, and only as wide as it is given (`w`, `grow`, `w_full`). Long text scrolls
/// sideways to keep the caret in sight. Caret moves, selection and the caret's blink never
/// run the view.
pub fn text_input<S>(key: impl Into<Key>) -> El<S> {
    field(key.into(), false)
}

/// A text field of several lines named `key`, wrapping at word boundaries; Enter starts a new
/// line (Ctrl+Enter submits). Text taller than the box scrolls in it like a scroll view (the
/// wheel, the overlay scrollbar, the caret kept in sight). [`El::rows`] lines tall (3) unless
/// sized. Otherwise as [`text_input`].
pub fn text_area<S>(key: impl Into<Key>) -> El<S> {
    field(key.into(), true)
}

/// A vector icon filling a square box, [`super::token::ICON`] across unless sized.
pub fn icon<S>(icon: Icon) -> El<S> {
    new(What::Icon(icon))
}

/// A window onto a native layer the host draws with its own GPU device (a 3D viewport, a
/// video): the element clears its box to transparent ([`crate::Op::Hole`]) and
/// [`Runtime::natives`](super::Runtime::natives) reports where it shows, so the host places
/// the surface's layer `name` there ([`crate::Surface::native_layer`]). It has no size of
/// its own (size it, or `grow`). Anything drawn after it, such as overlays and children,
/// draws over the layer; give it `on_pointer`, `on_wheel`, `focusable` and `on_key` for
/// its input.
pub fn native<S>(name: impl Into<Key>) -> El<S> {
    new(What::Native(name.into()))
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

/// A circular arc filled `fraction` (0..=1) of the way round, clockwise from twelve o'clock,
/// over a track: a ring or a meter. It is a circle fitting its box ([`super::token::ICON`]
/// across unless sized), stroked [`super::token::RING`] wide with round caps; its ink is
/// [`El::color`] ([`super::token::ACCENT`] by default), its track [`El::track`]. With
/// [`El::motion`] the fraction springs too.
pub fn arc<S>(fraction: f32) -> El<S> {
    new(What::Arc(ArcLook {
        fraction: if fraction.is_nan() { 0.0 } else { fraction.clamp(0.0, 1.0) },
        start: 0.0,
        span: 1.0,
        stroke: super::token::RING,
        track: Some(super::token::FILL),
    }))
}

impl<S> El<S> {
    /// Names the element among its siblings. Keyed elements keep their identity when
    /// siblings come and go; give one to everything a script, test or animation refers to.
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Child.
    pub fn child(mut self, child: El<S>) -> Self {
        self.children.push(child);
        self
    }

    /// Children.
    pub fn children(mut self, children: impl IntoIterator<Item = El<S>>) -> Self {
        self.children.extend(children);
        self
    }

    // ---- Size and flex --------------------------------------------------------------------

    /// W.
    pub fn w(mut self, w: impl Into<Len>) -> Self {
        self.sizing.w = Some(Dim::Len(w.into()));
        self
    }

    /// H.
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

    /// Min w.
    pub fn min_w(mut self, v: impl Into<Len>) -> Self {
        self.sizing.min_w = Some(v.into());
        self
    }

    /// Min h.
    pub fn min_h(mut self, v: impl Into<Len>) -> Self {
        self.sizing.min_h = Some(v.into());
        self
    }

    /// Max w.
    pub fn max_w(mut self, v: impl Into<Len>) -> Self {
        self.sizing.max_w = Some(v.into());
        self
    }

    /// Max h.
    pub fn max_h(mut self, v: impl Into<Len>) -> Self {
        self.sizing.max_h = Some(v.into());
        self
    }

    /// Space between children.
    pub fn gap(mut self, gap: impl Into<Len>) -> Self {
        self.sizing.gap = Some(gap.into());
        self
    }

    /// Lets a box's children flow onto further lines (rows below, or columns beside) when
    /// they overflow its main axis, `gap` apart both ways: a tile grid whose column count
    /// follows the width.
    pub fn flow_wrap(mut self) -> Self {
        self.sizing.flow_wrap = true;
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

    /// The theme's soft shadow under the box, at the default level
    /// ([`crate::ShadowLevel::Raised`]).
    pub fn shadow(self) -> Self {
        self.shadow_level(crate::ShadowLevel::default())
    }

    /// The theme's soft shadow of `level` (a menu's, a dialog's...) under the box.
    pub fn shadow_level(mut self, level: crate::ShadowLevel) -> Self {
        self.look.shadow = Some(Cast::Level(level));
        self
    }

    /// A custom soft shadow under the box, design px.
    pub fn box_shadow(mut self, shadow: crate::SoftShadow) -> Self {
        self.look.shadow = Some(Cast::Custom(shadow));
        self
    }

    /// Fades the element and everything in it.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.look.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// A text element's (or field's) role in the theme's type ramp.
    pub fn font(mut self, role: TextRole) -> Self {
        match &mut self.what {
            What::Text { face, .. } => face.role = role,
            What::Field(f) => f.face.role = role,
            _ => {}
        }
        self
    }

    // ---- Text fields ----------------------------------------------------------------------

    fn spec(&mut self, set: impl FnOnce(&mut FieldSpec)) -> &mut Self {
        if let What::Field(f) = &mut self.what {
            set(f);
        }
        self
    }

    /// A field's hint while it is empty, in the theme's tertiary ink. Assistive technology
    /// reads it as the name when the field has no [`label`](Self::label).
    pub fn placeholder(mut self, text: impl Into<Arc<str>>) -> Self {
        let text = text.into();
        self.spec(|f| f.placeholder = Some(text));
        self
    }

    /// A password field: bullets on screen, no copy or cut, and no text for assistive
    /// technology.
    pub fn secure(mut self) -> Self {
        self.spec(|f| f.secure = true);
        self
    }

    /// The most characters a field takes; typing and pasting stop there.
    pub fn max_len(mut self, chars: usize) -> Self {
        self.spec(|f| f.max_len = Some(chars));
        self
    }

    /// A field that can be focused, selected and copied, but not changed.
    pub fn read_only(mut self) -> Self {
        self.spec(|f| f.read_only = true);
        self
    }

    /// Selects a field's whole text whenever it gets focus (a search or rename field).
    pub fn select_all_on_focus(mut self) -> Self {
        self.spec(|f| f.select_all = true);
        self
    }

    /// A [`text_area`]'s height in lines when it isn't sized (3 by default).
    pub fn rows(mut self, rows: usize) -> Self {
        self.spec(|f| f.rows = rows.max(1));
        self
    }

    /// Runs on `&mut S` with a field's new text after each edit (typing, deleting, pasting,
    /// undo, an IME commit): `on_input(|s, text| s.name = text.into())`. The view runs again,
    /// and what it gives [`value`](Self::value) is what the field shows: give the old text
    /// back to refuse the edit, or another to change it.
    pub fn on_input(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
        self.on_input = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` with a field's text when Enter goes down in a [`text_input`] (Ctrl+Enter
    /// in a [`text_area`]). Without it, Enter goes on to the field's ancestors.
    pub fn on_submit(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
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

    /// Extra help assistive technology reads after the label ("Applies to every display").
    pub fn description(mut self, description: impl Into<Arc<str>>) -> Self {
        self.sem.description = Some(description.into());
        self
    }

    /// The value as assistive technology reads it ("40%", "Medium"), for a range or any
    /// control showing a value. On a [`text_input`] or [`text_area`], its text (which is also
    /// what assistive technology reads, unless it is [`secure`](Self::secure)).
    pub fn value(mut self, value: impl Into<Arc<str>>) -> Self {
        let value = value.into();
        match &mut self.what {
            What::Field(f) => f.value = Some(value),
            _ => self.sem.value = Some(value),
        }
        self
    }

    /// For the members of a list, tab bar or segmented control: whether this one is chosen.
    /// A focus group's Tab lands on its selected (or checked) member.
    pub fn selected(mut self, selected: bool) -> Self {
        self.sem.selected = Some(selected);
        self
    }

    /// For disclosures (a menu button, a section): whether what it shows is open.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.sem.expanded = Some(expanded);
        self
    }

    /// Whether its value fails validation (a field with an error): assistive technology says
    /// it is invalid. Pair it with a [`description`](Self::description) saying why.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.sem.invalid = invalid;
        self
    }

    /// The element keyed `key` that this one shows as active while it keeps focus itself: the
    /// highlighted row of a combobox's or search field's list, or of a list moved through by
    /// its own keys. Assistive technology reads that row as if it had focus. The first element
    /// keyed `key` anywhere in the view counts (a list in an overlay too); one with no
    /// semantics of its own gets none from this.
    pub fn active_descendant(mut self, key: impl Into<Key>) -> Self {
        self.sem.active = Some(key.into());
        self
    }

    /// A value from `min` to `max` in steps of `step` (sliders, steppers): announced as a
    /// range, and stepped from the keyboard and assistive technology through
    /// [`on_step`](Self::on_step). The role defaults to a slider.
    pub fn range(mut self, min: f64, max: f64, step: f64, value: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let step = if step.is_finite() && step > 0.0 { step } else { (max - min) / 100.0 };
        let value = if value.is_nan() { min } else { value.clamp(min, max) };
        self.sem.range = Some(Range { min, max, step, value });
        self
    }

    /// Makes the element a roving focus group: Tab stops on it once (on its selected or
    /// checked member, else the first), arrows along `orientation` move between its enabled
    /// members, wrapping, and Home and End go to the ends. Members are the focusable elements
    /// inside it (not those inside a nested group).
    pub fn focus_group(mut self, orientation: Orientation) -> Self {
        self.group = Some(Group {
            axis: orientation,
            follow: self.group.is_some_and(|g| g.follow),
        });
        self
    }

    /// In a [`focus_group`](Self::focus_group), arrows also press the member they land on, so
    /// selection follows focus (radio groups, segmented controls).
    pub fn select_follows_focus(mut self) -> Self {
        let axis = self.group.map_or(Axis::Horizontal, |g| g.axis);
        self.group = Some(Group { axis, follow: true });
        self
    }

    // ---- Scrolling ------------------------------------------------------------------------

    /// Makes a box a scroll view along `axis`: its children lay out at their own length on
    /// that axis and are clipped to the box (hits too). The wheel, Page Up and Page Down
    /// (while focus is inside), and arrows, Space, Home and End (while the view itself has
    /// focus) scroll it along the theme's scroll spring; keyboard focus moving onto a hidden
    /// child scrolls it into view. An overlay scrollbar shows while it scrolls and fades
    /// after the theme's `scrollbar_idle`. Scrolling never runs the view. Give the box a size
    /// (`h`, `grow`, `max_h`) for it to be smaller than its content.
    pub fn scroll(mut self, axis: Axis) -> Self {
        self.scroll = Some(axis);
        self
    }

    /// Keeps a scroll view at its end while content grows, as long as it rests there: a log
    /// or a chat shows what arrived, and scrolling back holds the place until the person
    /// scrolls to the end again. Works with [`super::virtual_list`].
    pub fn follow_end(mut self) -> Self {
        self.follow_end = true;
        self
    }

    // ---- Arcs -----------------------------------------------------------------------------

    /// Where an [`arc`] starts; it runs clockwise from there.
    pub fn arc_start(self, start: ArcStart) -> Self {
        self.arc_angle(start.angle())
    }

    /// Where an [`arc`] starts, in radians clockwise from twelve o'clock: `1.25 * PI` is
    /// half past seven, the start of a gauge spanning three quarters (`.arc_span(0.75)`).
    pub fn arc_angle(mut self, radians: f32) -> Self {
        if let What::Arc(a) = &mut self.what {
            a.start = if radians.is_finite() { radians.rem_euclid(std::f32::consts::TAU) } else { 0.0 };
        }
        self
    }

    /// How much of a turn an [`arc`] spans (its track), `0..=1`: `0.5` from
    /// [`ArcStart::Left`] is a semicircle meter. A whole turn by default.
    pub fn arc_span(mut self, span: f32) -> Self {
        if let What::Arc(a) = &mut self.what {
            a.span = if span.is_nan() { 1.0 } else { span.clamp(0.0, 1.0) };
        }
        self
    }

    /// An [`arc`]'s stroke width ([`super::token::RING`] by default).
    pub fn stroke(mut self, width: impl Into<Len>) -> Self {
        if let What::Arc(a) = &mut self.what {
            a.stroke = width.into();
        }
        self
    }

    /// An [`arc`]'s track colour ([`super::token::FILL`] by default).
    pub fn track(mut self, track: impl Into<Paint>) -> Self {
        if let What::Arc(a) = &mut self.what {
            a.track = Some(track.into());
        }
        self
    }

    /// An [`arc`] with no track: only its filled part draws.
    pub fn no_track(mut self) -> Self {
        if let What::Arc(a) = &mut self.what {
            a.track = None;
        }
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

    /// Whether it draws its own keyboard focus ring (on by default). Turn it off where a
    /// container shows the focus instead, through [`focus_within`](Self::focus_within): a
    /// search box's border lighting up around its field, so the field isn't ringed twice.
    /// Focus, Tab order and assistive technology are unchanged.
    pub fn focus_ring(mut self, on: bool) -> Self {
        self.quiet = !on;
        self
    }

    /// A look it takes while keyboard focus is on it or anywhere inside it (CSS's
    /// `:focus-within`): `.focus_within(|el| el.fill(token::FILL).radius(8.0))`. The fill,
    /// colour, radius, shadow and opacity (below 1) set in `style` replace its own; the rest
    /// stays. Focus moving in and out repaints without running the view, and the change
    /// springs along the element's [`motion`](Self::motion) like any other.
    pub fn focus_within(mut self, style: impl FnOnce(El<S>) -> El<S>) -> Self {
        self.states.within = Some(style(el()).look);
        self
    }

    /// Its ink (its `color`, or a state look's) flows to every descendant that sets none, as
    /// CSS's `color` inherits: a button's label and icon follow the button's hover colour,
    /// springing with it. Inside, an element that sets its own ink passes that one on.
    /// Without it, text and icons take the theme's label ink.
    pub fn cascade_ink(mut self) -> Self {
        self.cascade = true;
        self
    }

    /// A look it takes while the pointer is over it or anything inside it (CSS's `:hover`):
    /// `.hovered(|el| el.fill(token::FILL).color(token::LABEL))`. Like
    /// [`focus_within`](Self::focus_within), its fill, colour, border, radius, shadow and
    /// opacity replace its own while hovered, springing along its motion, without running
    /// the view. Pressed and focus looks lie over it.
    pub fn hovered(mut self, style: impl FnOnce(El<S>) -> El<S>) -> Self {
        self.states.hover = Some(style(el()).look);
        self
    }

    /// Runs on `&mut S` with the paths of files dropped on it from another app (Finder,
    /// Explorer). The innermost element with one under the drop takes it; it needs no other
    /// handler. Pair it with [`dropping`](Self::dropping) to show where files will land.
    pub fn on_drop_files(mut self, handler: impl Fn(&mut S, &[std::path::PathBuf]) + 'static) -> Self {
        self.on_files = Some(Box::new(handler));
        self
    }

    /// A look it takes while dragged files are over it and it would take them
    /// ([`on_drop_files`](Self::on_drop_files)): a highlighted border, a tinted fill.
    pub fn dropping(mut self, style: impl FnOnce(El<S>) -> El<S>) -> Self {
        self.states.drop = Some(style(el()).look);
        self
    }

    /// A look it takes while it is pressed (CSS's `:active`), over its hover and focus looks.
    pub fn pressed(mut self, style: impl FnOnce(El<S>) -> El<S>) -> Self {
        self.states.press = Some(style(el()).look);
        self
    }

    /// A border `width` design px wide, drawn inside its edge and following its radius. Its
    /// colour springs with the element's motion like its fill; state looks can change it
    /// (`.focus_within(|el| el.border(1.0, token::ACCENT))`).
    pub fn border(mut self, width: impl Into<Len>, paint: impl Into<Paint>) -> Self {
        self.look.border = Some(Border {
            width: width.into(),
            paint: paint.into(),
        });
        self
    }

    /// The pointer's shape while it is over the element (the innermost element that sets one
    /// wins), and while a press that began on it lasts. Text fields show [`Cursor::Text`]
    /// unless they set another. Hosts read it with [`Runtime::cursor`](super::Runtime::cursor).
    ///
    /// [`Cursor::Text`]: crate::Cursor::Text
    pub fn cursor(mut self, cursor: crate::Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Text in `family` instead of the theme's (a monospaced face for values and code).
    /// Families the system lacks fall back to the theme's.
    pub fn family(mut self, family: impl Into<Arc<str>>) -> Self {
        if let Some(face) = self.face_mut() {
            face.family = Some(family.into());
        }
        self
    }

    /// Text at this weight (100..=900) instead of its role's.
    pub fn weight(mut self, weight: u16) -> Self {
        if let Some(face) = self.face_mut() {
            face.weight = Some(weight.clamp(1, 1000));
        }
        self
    }

    /// Text at this size in design px instead of its role's; its line height and tracking
    /// scale with it.
    pub fn text_size(mut self, size: f32) -> Self {
        if let Some(face) = self.face_mut() {
            face.size = Some(size.max(1.0));
        }
        self
    }

    /// Where a line of text sits across its box, when the box is wider than the text.
    pub fn text_align(mut self, align: TextAlign) -> Self {
        if let Some(face) = self.face_mut() {
            face.align = align;
        }
        self
    }

    fn face_mut(&mut self) -> Option<&mut Face> {
        match &mut self.what {
            What::Text { face, .. } => Some(face),
            What::Field(f) => Some(&mut f.face),
            _ => None,
        }
    }

    /// Turns the element and everything in it `radians` clockwise about its centre, at draw
    /// time: layout keeps its upright box, while what it draws, its hits (their bounding box)
    /// and its damage are turned. With [`motion`](Self::motion) the angle springs (it snaps
    /// under reduced motion): a disclosure chevron, `.rotate(if open { FRAC_PI_2 } else { 0.0 })`.
    /// Arcs, lines, circles and icons turn exactly (an icon is drawn as its stroked outline
    /// while turned; filled icons stay upright), and boxes at quarter turns; text, images
    /// and boxes at other angles keep upright, about the turned centre.
    pub fn rotate(mut self, radians: f32) -> Self {
        self.anim.rotate = if radians.is_finite() { radians } else { 0.0 };
        self
    }

    /// Spins the element clockwise, one turn every `period`, on top of its
    /// [`rotate`](Self::rotate): a spinner (an [`arc`] with a short span), indeterminate
    /// progress. It moves only while it is visible: laid out, not scrolled out of its scroll
    /// views, not under an open dialog, its window shown ([`super::Runtime::set_visible`]),
    /// and not under reduced motion (where it holds still at its rest angle). Frames run only
    /// while something visible spins; the view never runs for it.
    pub fn spin(mut self, period: Duration) -> Self {
        self.anim.spin = (!period.is_zero()).then_some(period);
        self
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

    /// Runs on `&mut S` for the pointer's raw story on the element: `Hover` while it moves
    /// over with no button held, then `Down`, `Move`… and `Up` (or `Cancel`) for a press of
    /// any button that began on it, which holds the pointer until released, wherever it goes.
    /// Each event has its place, its delta since the last (moves within a frame add up, so a
    /// fast drag runs the handler once a frame), the press's origin, the modifiers and the
    /// click count: number scrubbing, splitters, tab and row drags, canvases.
    ///
    /// Return whether the state changed: `false` (a hover that changed nothing) runs no view.
    /// Changes land at once, with no motion, as a drag's do. Presses still reach `on_press`.
    pub fn on_pointer(mut self, handler: impl Fn(&mut S, &PointerEvent, &mut Tx) -> bool + 'static) -> Self {
        self.on_pointer = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` when the wheel turns over the element (or anything inside it without
    /// its own): the element takes the wheel, so scroll views around it stay put. Return
    /// whether the state changed: zooming a canvas, nudging a number.
    pub fn on_wheel(mut self, handler: impl Fn(&mut S, &WheelEvent, &mut Tx) -> bool + 'static) -> Self {
        self.on_wheel = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` when a key goes down while focus is on the element or inside it, the
    /// innermost first: `on_key(|s, key, tx| match key { KeyCode::DELETE => { ..; Handled::Yes }
    /// _ => Handled::No })`. A used key re-runs the view and goes no further; an unused one
    /// goes on to the element's built-in keys (a range, a group, a scroll view), then its
    /// parent, then the defaults (Enter and Space press, arrows move focus). Return
    /// `Handled::No` without changing the state. It runs as the key arrives, after any
    /// handlers already queued. A key handler alone does not make an element focusable.
    pub fn on_key(mut self, handler: impl Fn(&mut S, KeyCode, &mut Tx) -> Handled + 'static) -> Self {
        self.on_key = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` when a character is typed while focus is on the element or inside it,
    /// the innermost first, after a focused text field (which takes every character it can):
    /// type-ahead in a list, `/` opening a search. Like [`on_key`](Self::on_key), a used
    /// character re-runs the view and goes no further.
    pub fn on_char(mut self, handler: impl Fn(&mut S, char, &mut Tx) -> Handled + 'static) -> Self {
        self.on_char = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` when the keyboard or assistive technology steps a
    /// [`range`](Self::range), with the change in value (the new value less the shown one,
    /// within `min..=max`): Right and Up add a step, Left and Down take one, Page Up and Page
    /// Down a tenth of the range (whole steps), Home and End go to the ends. Add it to the
    /// state: `on_step(|s, delta| s.level += delta)`. The view then runs again.
    pub fn on_step(mut self, handler: impl Fn(&mut S, f64) + 'static) -> Self {
        self.on_step = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` when the secondary button is released over the element (the innermost
    /// one with a handler), or the Menu key goes down while focus is in it, with where a menu
    /// should open (the pointer, or the focused element's centre), design px in the window:
    /// `on_context_menu(|s, x, y| s.menu = Some((x, y)))` with
    /// `.overlay(s.menu.map(|(x, y)| menu(items).at(x, y)))`. It makes nothing pressable.
    pub fn on_context_menu(mut self, handler: impl Fn(&mut S, f32, f32) + 'static) -> Self {
        self.on_context = Some(Box::new(handler));
        self
    }

    /// Runs on `&mut S` once, `after` the element first appears (again only if it leaves and
    /// comes back): a toast timing out. The host wakes for it and nothing else; once the
    /// element goes, nothing stays armed.
    pub fn on_timeout(mut self, after: Duration, handler: impl Fn(&mut S) + 'static) -> Self {
        self.on_timeout = Some((after, Box::new(handler)));
        self
    }

    // ---- Overlays -------------------------------------------------------------------------

    /// Declares an overlay ([`super::popover`], [`super::menu`], [`super::dialog`],
    /// [`super::toasts`]) with this element as its trigger: it is placed against the element
    /// unless it names another anchor, and drawn above the window's content while it is
    /// [`open`](super::Overlay::open) (and `Some`). The element tells assistive technology
    /// what it opens, whether that is open, and which panel it controls. Call it again for
    /// more overlays.
    pub fn overlay(mut self, overlay: impl Into<Option<super::Overlay<S>>>) -> Self {
        if let Some(overlay) = overlay.into() {
            self.popup = self.popup.or(overlay.kind.popup());
            self.overlays.push(overlay);
        }
        self
    }

    /// A tooltip: `text` opens above the element once the pointer rests on it (or keyboard
    /// focus arrives) for [`super::TOOLTIP_DELAY`], and closes when it leaves, on a press or on
    /// Escape. It takes neither focus nor the pointer, and opening it doesn't run the view.
    /// Assistive technology reads it as the element's description (unless it has one).
    pub fn tooltip(mut self, text: impl Into<Arc<str>>) -> Self {
        self.tip = Some(text.into());
        self
    }

    /// Whether input lands on it (it takes a hit item that can be enabled).
    pub(crate) fn interactive(&self) -> bool {
        self.on_press.is_some()
            || self.on_drag.is_some()
            || self.on_pointer.is_some()
            || self.on_wheel.is_some()
            || self.on_step.is_some()
            || self.sem.focusable
            || self.scroll.is_some() && matches!(self.what, What::Box(_))
            || self.field().is_some()
    }

    /// Its options, if it is a text field.
    pub(crate) fn field(&self) -> Option<&FieldSpec> {
        match &self.what {
            What::Field(f) => Some(f),
            _ => None,
        }
    }

    /// Whether it is a scroll view (only boxes scroll).
    pub(crate) fn scrolls(&self) -> Option<Axis> {
        self.scroll.filter(|_| matches!(self.what, What::Box(_)))
    }

    /// Whether it gets a hit item at all: interactive, or carrying semantics of its own.
    pub(crate) fn hit(&self) -> bool {
        self.interactive() || self.sem.role.is_some() || self.sem.label.is_some() || self.tip.is_some()
    }

    /// The first text in it, for a default label.
    pub(crate) fn first_text(&self) -> Option<&Arc<str>> {
        match &self.what {
            What::Text { text, .. } => Some(text),
            _ => self.children.iter().find_map(El::first_text),
        }
    }
}
