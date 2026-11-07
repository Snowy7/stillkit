//! Theme-driven components built only from public elements. Model values are supplied by views.
use std::sync::Arc;
use stillkit::el::{
    Align, El, Justify, Len, Orientation, Paint, Transition, col, el, grid, icon, row, text, token,
};
pub use stillkit::icons::Icon;
use stillkit::{Rgba, Role, TextRole, Theme};

/// Resolve contrast against a component's own composited surface.
pub fn readable(ink: Rgba, surface: Rgba, floor: f32) -> Rgba {
    use stillkit::theme::contrast;
    if contrast(ink, surface) >= floor {
        return ink;
    }
    let black = Rgba::new(0., 0., 0., 1.);
    let white = Rgba::new(1., 1., 1., 1.);
    let target = if contrast(black, surface) > contrast(white, surface) {
        black
    } else {
        white
    };
    for step in 1..=100 {
        let a = step as f32 / 100.;
        let c = Rgba::new(
            ink.r + (target.r - ink.r) * a,
            ink.g + (target.g - ink.g) * a,
            ink.b + (target.b - ink.b) * a,
            1.,
        );
        if contrast(c, surface) >= floor {
            return c;
        }
    }
    target
}
/// Composite the surface over the preset's scheme-relative headless backdrop.
pub fn surface(t: &Theme, fill: Rgba) -> Rgba {
    stillkit::theme::over(fill, t.color.surface.with_alpha(1.))
}
// Text must survive every neutral component surface, including nested cards and fields.
fn neutral_ink(t: &Theme, mut ink: Rgba, floor: f32) -> Rgba {
    for fill in [
        Rgba::TRANSPARENT,
        t.color.card,
        t.color.fill,
        t.color.well,
        t.color.elevated,
    ] {
        ink = readable(ink, surface(t, fill), floor);
    }
    ink
}
const LABEL: Paint = Paint::Theme(|t| neutral_ink(t, t.color.label, 7.));
const SECONDARY: Paint = Paint::Theme(|t| neutral_ink(t, t.color.secondary, 4.5));
fn part(key: &Option<Arc<str>>, name: &str) -> Arc<str> {
    key.as_ref()
        .map_or_else(|| name.into(), |k| format!("{k}/{name}").into())
}
type Press<S> = Box<dyn Fn(&mut S)>;
type Change<S, V> = Arc<dyn Fn(&mut S, V)>;
macro_rules! actions {
    () => {
        /// Stable caller-owned identity.
        pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
            self.key = Some(key.into());
            self
        }
        /// Keep semantics while ignoring input.
        pub fn disabled(mut self, v: bool) -> Self {
            self.disabled = v;
            self
        }
        /// Activation runs between frames on the model.
        pub fn on_press(mut self, f: impl Fn(&mut S) + 'static) -> Self {
            self.press = Some(Box::new(f));
            self
        }
    };
}
fn action<S: 'static>(
    mut node: El<S>,
    key: Option<Arc<str>>,
    disabled: bool,
    press: Option<Press<S>>,
) -> El<S> {
    if let Some(k) = key {
        node = node.key(k);
    }
    if let Some(f) = press {
        node = node.on_press(f);
    }
    node.motion(token::MOTION_INTERACTIVE)
        .disabled(disabled)
        .opacity(if disabled { 0.45 } else { 1. })
}
/// Button emphasis; unrelated to look selection.
#[derive(Clone, Copy, Debug, Default)]
pub enum ButtonKind {
    #[default]
    Filled,
    Tinted,
    Gray,
    Plain,
    Destructive,
}
/// Accent fill adjusted where neither black nor white can reach 7:1.
pub fn button_surface(t: &Theme) -> Rgba {
    let mut c = t.color.accent;
    for _ in 0..100 {
        let s = surface(t, c);
        if stillkit::theme::contrast(Rgba::new(0., 0., 0., 1.), s)
            .max(stillkit::theme::contrast(Rgba::new(1., 1., 1., 1.), s))
            >= 7.
        {
            break;
        }
        c = Rgba::new(c.r * 0.98, c.g * 0.98, c.b * 0.98, 1.);
    }
    c
}
/// Action with optional leading icon.
pub struct Button<S> {
    label: Arc<str>,
    key: Option<Arc<str>>,
    kind: ButtonKind,
    glyph: Option<Icon>,
    disabled: bool,
    press: Option<Press<S>>,
}
impl<S: 'static> Button<S> {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            key: None,
            kind: ButtonKind::Filled,
            glyph: None,
            disabled: false,
            press: None,
        }
    }
    actions!();
    pub fn kind(mut self, v: ButtonKind) -> Self {
        self.kind = v;
        self
    }
    pub fn icon(mut self, v: Icon) -> Self {
        self.glyph = Some(v);
        self
    }
    pub fn build(self) -> El<S> {
        let (fill, ink): (Paint, Paint) = match self.kind {
            ButtonKind::Filled => (
                Paint::Theme(button_surface),
                Paint::Theme(|t| readable(t.color.on_accent, surface(t, button_surface(t)), 7.)),
            ),
            ButtonKind::Tinted => (
                Paint::Theme(|t| t.color.accent_wash),
                Paint::Theme(|t| readable(t.color.accent, surface(t, t.color.accent_wash), 7.)),
            ),
            ButtonKind::Gray => (
                token::FILL,
                Paint::Theme(|t| readable(t.color.label, surface(t, t.color.fill), 7.)),
            ),
            ButtonKind::Plain => (
                Rgba::TRANSPARENT.into(),
                Paint::Theme(|t| readable(t.color.accent, surface(t, Rgba::TRANSPARENT), 7.)),
            ),
            ButtonKind::Destructive => (
                token::FILL,
                Paint::Theme(|t| readable(t.color.destructive, surface(t, t.color.fill), 7.)),
            ),
        };
        let mut n = row()
            .h(token::CONTROL)
            .pad_x(14.)
            .gap(token::ITEM)
            .align(Align::Center)
            .justify(Justify::Center)
            .radius(token::RADIUS_CONTROL)
            .fill(fill)
            .role(Role::Button)
            .label(self.label.clone());
        if let Some(g) = self.glyph {
            n = n.child(
                icon(g)
                    .key(part(&self.key, "icon"))
                    .size(token::ICON)
                    .color(ink),
            );
        }
        n = n.child(
            text(self.label)
                .key(part(&self.key, "label"))
                .font(TextRole::Headline)
                .color(ink),
        );
        action(n, self.key, self.disabled, self.press)
    }
}
/// Icon action with a required accessible label.
pub struct IconButton<S> {
    inner: Button<S>,
}
impl<S: 'static> IconButton<S> {
    pub fn new(g: Icon, label: impl Into<Arc<str>>) -> Self {
        Self {
            inner: Button::new(label).icon(g),
        }
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.inner = self.inner.key(k);
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.inner = self.inner.disabled(v);
        self
    }
    pub fn on_press(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_press(f);
        self
    }
    pub fn build(self) -> El<S> {
        let b = self.inner;
        let n = row()
            .size(token::CONTROL)
            .radius(token::RADIUS_CONTROL)
            .fill(token::FILL)
            .align(Align::Center)
            .justify(Justify::Center)
            .role(Role::Button)
            .label(b.label)
            .child(
                icon(b.glyph.unwrap())
                    .key(part(&b.key, "icon"))
                    .size(token::ICON)
                    .color(LABEL),
            );
        action(n, b.key, b.disabled, b.press)
    }
}
/// Binary switch with model-owned checked state.
pub struct Switch<S> {
    label: Arc<str>,
    on: bool,
    key: Option<Arc<str>>,
    disabled: bool,
    press: Option<Press<S>>,
}
impl<S: 'static> Switch<S> {
    pub fn new(label: impl Into<Arc<str>>, on: bool) -> Self {
        Self {
            label: label.into(),
            on,
            key: None,
            disabled: false,
            press: None,
        }
    }
    actions!();
    pub fn on_toggle(self, f: impl Fn(&mut S) + 'static) -> Self {
        self.on_press(f)
    }
    pub fn build(self) -> El<S> {
        let n = el()
            .w(40.)
            .h(24.)
            .radius(12.)
            .fill(if self.on { token::ACCENT } else { token::FILL })
            .role(Role::Switch)
            .label(self.label)
            .checked(self.on)
            .child(
                el().key(part(&self.key, "thumb"))
                    .size(20.)
                    .radius(10.)
                    .fill(token::KNOB)
                    .absolute(if self.on { 18. } else { 2. }, 2.),
            );
        action(n, self.key, self.disabled, self.press)
    }
}
/// Checkbox with filled selection, no resting outline.
pub struct Checkbox<S> {
    inner: Switch<S>,
}
impl<S: 'static> Checkbox<S> {
    pub fn new(label: impl Into<Arc<str>>, on: bool) -> Self {
        Self {
            inner: Switch::new(label, on),
        }
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.inner = self.inner.key(k);
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.inner = self.inner.disabled(v);
        self
    }
    pub fn on_toggle(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_toggle(f);
        self
    }
    pub fn build(self) -> El<S> {
        let s = self.inner;
        let mut mark = el()
            .motion(token::MOTION_INTERACTIVE)
            .size(20.)
            .radius(token::RADIUS_SMALL)
            .fill(if s.on { token::ACCENT } else { token::FILL });
        if s.on {
            mark = mark.child(
                icon(Icon::Check)
                    .key(part(&s.key, "check"))
                    .transition(Transition::scale(0.6))
                    .size(16.)
                    .absolute(2., 2.)
                    .color(token::ON_ACCENT),
            );
        }
        let n = row()
            .min_h(token::HIT)
            .gap(token::ITEM)
            .align(Align::Center)
            .radius(token::RADIUS_SMALL)
            .role(Role::CheckBox)
            .label(s.label.clone())
            .checked(s.on)
            .child(mark.key(part(&s.key, "mark")))
            .child(text(s.label).color(LABEL).key(part(&s.key, "label")));
        action(n, s.key, s.disabled, s.press)
    }
}
fn unit(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0., 1.) } else { 0. }
}
fn track<S>(
    value: f32,
    height: f32,
    glyph: Option<Icon>,
    key: &Option<Arc<str>>,
    thumb: bool,
) -> El<S> {
    let mut fill = row()
        .h_full()
        .motion(token::MOTION_INTERACTIVE)
        .w(0.)
        .grow(value)
        .radius(999.)
        .fill(token::ACCENT)
        .key(part(key, "fill"));
    if let Some(g) = glyph {
        fill = fill
            .min_w(height)
            .child(icon(g).size(14.).color(token::ON_ACCENT).absolute(7., 7.));
    }
    // Zero bases keep the fill proportional instead of counting the label/thumb width.
    let mut n = row()
        .key(part(key, "track"))
        .h(height)
        .w_full()
        .radius(999.)
        .fill(token::FILL);
    n = n.child(fill);
    if thumb {
        n = n.align(Align::Center).child(
            el().key(part(key, "thumb"))
                .motion(token::MOTION_INTERACTIVE)
                .size(18.)
                .radius(9.)
                .fill(token::KNOB),
        );
    }
    n = n.child(el().w(0.).grow(1. - value));
    n
}
/// Slider in 0..=1. `icon` selects the thick Control Center bar.
pub struct Slider<S> {
    label: Arc<str>,
    value: f32,
    step: f32,
    glyph: Option<Icon>,
    key: Option<Arc<str>>,
    disabled: bool,
    change: Option<Change<S, f32>>,
}
impl<S: 'static> Slider<S> {
    pub fn new(label: impl Into<Arc<str>>, value: f32) -> Self {
        Self {
            label: label.into(),
            value: unit(value),
            step: 0.01,
            glyph: None,
            key: None,
            disabled: false,
            change: None,
        }
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn step(mut self, v: f32) -> Self {
        self.step = if v.is_finite() && v > 0. {
            v.min(1.)
        } else {
            0.01
        };
        self
    }
    pub fn icon(mut self, g: Icon) -> Self {
        self.glyph = Some(g);
        self
    }
    pub fn on_change(mut self, f: impl Fn(&mut S, f32) + 'static) -> Self {
        self.change = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let mut n = col()
            .h(token::CONTROL)
            .justify(Justify::Center)
            .radius(token::RADIUS_CONTROL)
            .role(Role::Slider)
            .label(format!("{} {}%", self.label, (self.value * 100.).round()))
            .range(0., 1., f64::from(self.step), f64::from(self.value))
            .child(track(
                self.value,
                if self.glyph.is_some() { 28. } else { 4. },
                self.glyph,
                &self.key,
                self.glyph.is_none(),
            ));
        if let Some(f) = self.change {
            let step = self.step;
            let change = f.clone();
            let value = self.value;
            let current = std::cell::Cell::new(f64::from(value));
            n = n.on_step(move |s, delta| {
                let next = (current.get() + delta).clamp(0., 1.);
                current.set(next);
                change(s, next as f32);
            });
            n = n.on_drag(move |s, x, _| f(s, ((x / step).round() * step).clamp(0., 1.)));
        }
        if let Some(k) = self.key {
            n = n.key(k);
        }
        n.disabled(self.disabled)
            .opacity(if self.disabled { 0.45 } else { 1. })
    }
}
/// Radio selection with one Tab stop and wrapping arrow navigation.
pub struct RadioGroup<S> {
    label: Arc<str>,
    options: Vec<Arc<str>>,
    selected: usize,
    key: Option<Arc<str>>,
    disabled: bool,
    change: Option<Change<S, usize>>,
    segmented: bool,
}
impl<S: 'static> RadioGroup<S> {
    pub fn new(
        label: impl Into<Arc<str>>,
        options: impl IntoIterator<Item = impl Into<Arc<str>>>,
        selected: usize,
    ) -> Self {
        Self {
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
            key: None,
            disabled: false,
            change: None,
            segmented: false,
        }
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn on_change(mut self, f: impl Fn(&mut S, usize) + 'static) -> Self {
        self.change = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let count = self.options.len();
        let selected = self.selected.min(count.saturating_sub(1));
        let mut root = if self.segmented {
            col()
                .pad(2.)
                .radius(token::RADIUS_CONTROL)
                .fill(token::WELL)
        } else {
            col().gap(token::ITEM)
        };
        root = root
            .role(Role::RadioGroup)
            .label(self.label)
            .focus_group(if self.segmented {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            })
            .select_follows_focus();
        if self.segmented && count > 0 {
            // A zero-height rail shares the options' width and draws the plate behind them.
            // Keep the plate under this stable parent so selection retargets its position.
            root = root.child(
                row()
                    .w_full()
                    .h(0.)
                    .align(Align::Start)
                    .child(el().w(0.).grow(selected as f32))
                    .child(
                        el().key(part(&self.key, "selection"))
                            .w(0.)
                            .grow(1.)
                            .h(token::CONTROL_SMALL)
                            .radius(token::RADIUS_CONTROL)
                            .motion(token::MOTION_INTERACTIVE)
                            .opacity(if self.disabled { 0.45 } else { 1. })
                            .fill(Paint::Theme(|t| {
                                if t.dark() { t.color.fill } else { t.color.knob }
                            })),
                    )
                    .child(el().w(0.).grow((count - selected - 1) as f32)),
            );
        }
        let mut options = if self.segmented {
            row()
        } else {
            col().gap(token::ITEM)
        };
        for (i, label) in self.options.into_iter().enumerate() {
            let key = Some(part(&self.key, &format!("option-{i}")));
            let mut n = row()
                .h(token::CONTROL_SMALL)
                .gap(token::ITEM)
                .pad_x(10.)
                .align(Align::Center)
                .justify(if self.segmented {
                    Justify::Center
                } else {
                    Justify::Start
                })
                .radius(token::RADIUS_CONTROL)
                .role(Role::RadioButton)
                .label(label.clone())
                .checked(i == self.selected)
                .selected(i == self.selected);
            if self.segmented {
                n = n.w(0.).min_w(0.).grow(1.);
            } else {
                let mut dot = el().size(18.).radius(9.).fill(token::FILL);
                if i == self.selected {
                    dot = dot.child(
                        el().key(part(&key, "dot"))
                            .size(10.)
                            .transition(Transition::scale(0.5))
                            .radius(5.)
                            .fill(token::ACCENT)
                            .absolute(4., 4.),
                    );
                }
                n = n.child(dot.key(part(&key, "mark")));
            }
            n = n.child(
                text(label)
                    .color(if !self.segmented || i == selected {
                        LABEL
                    } else {
                        SECONDARY
                    })
                    .key(part(&key, "label"))
                    .font(TextRole::Headline),
            );
            let f = self
                .change
                .clone()
                .map(|f| Box::new(move |s: &mut S| f(s, i)) as Press<S>);
            options = options.child(action(n, key, self.disabled, f));
        }
        root = root.child(options);
        if let Some(k) = self.key {
            root = root.key(k);
        }
        root
    }
}
/// Segmented single selection, sharing radio semantics.
pub struct Segmented<S>(RadioGroup<S>);
impl<S: 'static> Segmented<S> {
    pub fn new(
        label: impl Into<Arc<str>>,
        options: impl IntoIterator<Item = impl Into<Arc<str>>>,
        selected: usize,
    ) -> Self {
        let mut r = RadioGroup::new(label, options, selected);
        r.segmented = true;
        Self(r)
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.0 = self.0.key(k);
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.0 = self.0.disabled(v);
        self
    }
    pub fn on_change(mut self, f: impl Fn(&mut S, usize) + 'static) -> Self {
        self.0 = self.0.on_change(f);
        self
    }
    pub fn build(self) -> El<S> {
        self.0.build()
    }
}
/// Determinate progress announced as a percentage.
pub struct Progress {
    label: Arc<str>,
    value: f32,
}
impl Progress {
    pub fn new(label: impl Into<Arc<str>>, value: f32) -> Self {
        Self {
            label: label.into(),
            value: unit(value),
        }
    }
    pub fn build<S>(self) -> El<S> {
        track(self.value, 4., None, &None, false)
            .role(Role::ProgressIndicator)
            .label(format!("{} {}%", self.label, (self.value * 100.).round()))
            .range(0., 1., 0.01, f64::from(self.value))
    }
}
/// Small status label.
pub struct Badge(Arc<str>);
impl Badge {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self(label.into())
    }
    pub fn build<S>(self) -> El<S> {
        col().align(Align::Start).child(
            row()
                .motion(token::MOTION_INTERACTIVE)
                .pad_x(6.)
                .h(18.)
                .min_w(18.)
                .align(Align::Center)
                .justify(Justify::Center)
                .radius(9.)
                .fill(token::FILL)
                .role(Role::Label)
                .label(self.0.clone())
                .child(
                    text(self.0.clone())
                        .key(self.0)
                        .transition(Transition::slide(0., 4.))
                        .font(TextRole::Caption)
                        .color(Paint::Theme(|t| {
                            readable(t.color.label, surface(t, t.color.fill), 7.)
                        })),
                ),
        )
    }
}
/// List content with independently composable leading and accessory elements.
pub struct ListRow<S> {
    title: Arc<str>,
    subtitle: Option<Arc<str>>,
    lead: Option<El<S>>,
    accessory: Option<El<S>>,
    key: Option<Arc<str>>,
    disabled: bool,
    press: Option<Press<S>>,
}
impl<S: 'static> ListRow<S> {
    pub fn new(title: impl Into<Arc<str>>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            lead: None,
            accessory: None,
            key: None,
            disabled: false,
            press: None,
        }
    }
    actions!();
    pub fn subtitle(mut self, s: impl Into<Arc<str>>) -> Self {
        self.subtitle = Some(s.into());
        self
    }
    pub fn lead(mut self, e: El<S>) -> Self {
        self.lead = Some(e);
        self
    }
    pub fn accessory(mut self, e: El<S>) -> Self {
        self.accessory = Some(e);
        self
    }
    pub fn build(self) -> El<S> {
        let mut n = row()
            .min_h(if self.subtitle.is_some() {
                token::ROW
            } else {
                token::ROW_SINGLE
            })
            .pad_x(10.)
            .gap(token::ITEM)
            .align(Align::Center)
            .radius(token::RADIUS_CARD)
            .role(Role::ListItem)
            .label(self.subtitle.as_ref().map_or_else(
                || self.title.to_string(),
                |s| format!("{}: {s}", self.title),
            ));
        if let Some(e) = self.lead {
            n = n.child(e.key(part(&self.key, "lead")));
        }
        let mut titles = col().grow(1.).child(
            text(self.title)
                .color(LABEL)
                .key(part(&self.key, "title"))
                .font(TextRole::Headline),
        );
        if let Some(s) = self.subtitle {
            titles = titles.child(
                text(s)
                    .key(part(&self.key, "subtitle"))
                    .font(TextRole::Callout)
                    .color(SECONDARY),
            );
        }
        n = n.child(titles.key(part(&self.key, "content")));
        if let Some(e) = self.accessory {
            n = n.child(e.key(part(&self.key, "accessory")));
        }
        action(n, self.key, self.disabled, self.press)
    }
}
/// Control Center toggle tile.
pub struct Tile<S> {
    row: ListRow<S>,
    on: bool,
    glyph: Icon,
}
impl<S: 'static> Tile<S> {
    pub fn new(title: impl Into<Arc<str>>, glyph: Icon, on: bool) -> Self {
        Self {
            row: ListRow::new(title),
            on,
            glyph,
        }
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.row = self.row.key(k);
        self
    }
    pub fn subtitle(mut self, s: impl Into<Arc<str>>) -> Self {
        self.row = self.row.subtitle(s);
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.row = self.row.disabled(v);
        self
    }
    pub fn on_toggle(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.row = self.row.on_press(f);
        self
    }
    pub fn build(self) -> El<S> {
        let disc = row()
            .motion(token::MOTION_INTERACTIVE)
            .size(36.)
            .radius(18.)
            .fill(if self.on { token::ACCENT } else { token::FILL })
            .align(Align::Center)
            .justify(Justify::Center)
            .child(icon(self.glyph).size(18.).color(if self.on {
                token::ON_ACCENT
            } else {
                LABEL
            }));
        self.row
            .lead(disc)
            .build()
            .fill(token::CARD)
            .role(Role::Switch)
            .checked(self.on)
    }
}
/// Theme section heading.
pub struct SectionHeader(Arc<str>);
impl SectionHeader {
    pub fn new(s: impl Into<Arc<str>>) -> Self {
        Self(s.into())
    }
    pub fn build<S>(self) -> El<S> {
        text(self.0.clone())
            .font(TextRole::Subheadline)
            .color(SECONDARY)
            .role(Role::Heading)
            .label(self.0)
    }
}
/// Padded concentric card for arbitrary children.
pub struct Card<S>(El<S>);
impl<S: 'static> Default for Card<S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<S: 'static> Card<S> {
    pub fn new() -> Self {
        Self(
            col()
                .pad(token::PAD)
                .gap(token::GAP)
                .radius(token::RADIUS_CARD)
                .fill(token::CARD),
        )
    }
    pub fn child(mut self, e: El<S>) -> Self {
        self.0 = self.0.child(e);
        self
    }
    pub fn build(self) -> El<S> {
        self.0
    }
}
/// Theme separator with no interaction.
pub struct Separator;
impl Separator {
    pub fn build<S>(self) -> El<S> {
        el().h(1.)
            .w_full()
            .fill(token::SEPARATOR)
            .role(Role::Splitter)
            .label("Separator")
    }
}
/// Centred empty-state content.
pub struct EmptyState {
    title: Arc<str>,
    detail: Arc<str>,
    glyph: Icon,
}
impl EmptyState {
    pub fn new(title: impl Into<Arc<str>>, detail: impl Into<Arc<str>>, glyph: Icon) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            glyph,
        }
    }
    pub fn build<S>(self) -> El<S> {
        col()
            .pad(token::PAD)
            .gap(token::ITEM)
            .align(Align::Center)
            .role(Role::Group)
            .label(self.title.clone())
            .child(icon(self.glyph).size(32.).color(SECONDARY).key("icon"))
            .child(
                text(self.title)
                    .color(LABEL)
                    .font(TextRole::Title3)
                    .key("title"),
            )
            .child(text(self.detail).wrap(3).color(SECONDARY).key("detail"))
    }
}
/// Non-editing text field frame; focus comes from the runtime.
#[deprecated(note = "Use TextField for editable text")]
pub struct TextFieldFrame {
    label: Arc<str>,
    value: Arc<str>,
    placeholder: Arc<str>,
    disabled: bool,
}
#[allow(deprecated)]
impl TextFieldFrame {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            value: "".into(),
            placeholder: "".into(),
            disabled: false,
        }
    }
    pub fn value(mut self, s: impl Into<Arc<str>>) -> Self {
        self.value = s.into();
        self
    }
    pub fn placeholder(mut self, s: impl Into<Arc<str>>) -> Self {
        self.placeholder = s.into();
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn build<S>(self) -> El<S> {
        let empty = self.value.is_empty();
        row()
            .h(token::CONTROL)
            .pad_x(12.)
            .align(Align::Center)
            .radius(token::RADIUS_CONTROL)
            .fill(token::WELL)
            .role(Role::TextInput)
            .label(format!(
                "{}: {}",
                self.label,
                if empty {
                    &self.placeholder
                } else {
                    &self.value
                }
            ))
            .focusable()
            .disabled(self.disabled)
            .opacity(if self.disabled { 0.45 } else { 1. })
            .child(
                text(if empty { self.placeholder } else { self.value })
                    .key("value")
                    .color(if empty { SECONDARY } else { LABEL }),
            )
    }
}
/// Gregorian calendar month, Monday first, with model-owned selection.
pub struct CalendarMonth<S> {
    year: u16,
    month: u8,
    selected: Option<u8>,
    today: Option<u8>,
    navigate: Option<Change<S, (u16, u8)>>,
    key: Option<Arc<str>>,
    disabled: bool,
    change: Option<Change<S, u8>>,
}
impl<S: 'static> CalendarMonth<S> {
    pub fn new(year: u16, month: u8) -> Self {
        Self {
            year: year.clamp(1, 9999),
            month: month.clamp(1, 12),
            selected: None,
            today: None,
            navigate: None,
            key: None,
            disabled: false,
            change: None,
        }
    }
    /// Mark today's date when this is the current month; the host owns the clock.
    pub fn today(mut self, day: u8) -> Self {
        self.today = Some(day);
        self
    }
    /// Navigate by rebuilding from the returned (year, month); stops at years 1 and 9999.
    pub fn on_month_change(mut self, f: impl Fn(&mut S, (u16, u8)) + 'static) -> Self {
        self.navigate = Some(Arc::new(f));
        self
    }
    pub fn selected(mut self, d: u8) -> Self {
        self.selected = Some(d);
        self
    }
    pub fn key(mut self, k: impl Into<Arc<str>>) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn on_select(mut self, f: impl Fn(&mut S, u8) + 'static) -> Self {
        self.change = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let leap = self.year.is_multiple_of(4)
            && (!self.year.is_multiple_of(100) || self.year.is_multiple_of(400));
        let days = match self.month {
            2 => {
                if leap {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        let y = i32::from(self.year) - i32::from(self.month < 3);
        let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let start = (y + y / 4 - y / 100 + y / 400 + offsets[self.month as usize - 1] + 7) % 7;
        let mut cells = grid(7).gap(3.);
        for (i, d) in ["M", "T", "W", "T", "F", "S", "S"].into_iter().enumerate() {
            cells = cells.child(
                text(d)
                    .color(SECONDARY)
                    .key(format!("weekday-{i}"))
                    .font(TextRole::Caption),
            );
        }
        let previous_days = match self.month {
            3 => {
                if leap {
                    29
                } else {
                    28
                }
            }
            5 | 7 | 10 | 12 => 30,
            _ => 31,
        };
        let outside = |day: i32, key: String| {
            row()
                .key(key)
                .h(28.)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(
                    text(day.to_string())
                        .color(SECONDARY)
                        .font(TextRole::Callout),
                )
        };
        for i in 0..start {
            cells = cells.child(outside(
                previous_days - start + i + 1,
                format!("previous-{i}"),
            ));
        }
        for day in 1..=days {
            let selected = self.selected == Some(day);
            let ink = if selected {
                Paint::Theme(|t| readable(t.color.on_accent, surface(t, button_surface(t)), 7.))
            } else if self.today == Some(day) {
                Paint::Theme(|t| readable(t.color.accent, surface(t, Rgba::TRANSPARENT), 7.))
            } else {
                LABEL
            };
            let n = row()
                .h(28.)
                .justify(Justify::Center)
                .align(Align::Center)
                .radius(14.)
                .role(Role::RadioButton)
                .label(day.to_string())
                .checked(selected)
                .fill(if selected {
                    Paint::Theme(button_surface)
                } else {
                    Rgba::TRANSPARENT.into()
                })
                .child(text(day.to_string()).color(ink).font(TextRole::Callout));
            let f = self
                .change
                .clone()
                .map(|f| Box::new(move |s: &mut S| f(s, day)) as Press<S>);
            cells = cells.child(action(
                n,
                Some(part(&self.key, &format!("day-{day}"))),
                self.disabled,
                f,
            ));
        }
        for day in 1..=(7 - (start + i32::from(days)) % 7) % 7 {
            cells = cells.child(outside(day, format!("following-{day}")));
        }
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let heading = format!("{} {}", months[self.month as usize - 1], self.year);
        let mut header = row().align(Align::Center).child(
            text(heading.clone())
                .color(LABEL)
                .grow(1.)
                .key("heading")
                .font(TextRole::Headline),
        );
        for (forward, glyph, label) in [
            (false, Icon::ChevronLeft, "Previous month"),
            (true, Icon::ChevronRight, "Next month"),
        ] {
            let index = i32::from(self.year - 1) * 12
                + i32::from(self.month - 1)
                + if forward { 1 } else { -1 };
            let valid = (0..9999 * 12).contains(&index);
            let mut button = IconButton::new(glyph, label)
                .key(part(&self.key, if forward { "next" } else { "previous" }))
                .disabled(self.disabled || !valid);
            if valid && let Some(f) = self.navigate.clone() {
                button = button
                    .on_press(move |s| f(s, ((index / 12 + 1) as u16, (index % 12 + 1) as u8)));
            }
            header = header.child(button.build().size(24.).fill(Rgba::TRANSPARENT));
        }
        let mut n = col()
            .gap(token::ITEM)
            .role(Role::Group)
            .label(heading)
            .child(header)
            .child(cells.key("days"));
        if let Some(k) = self.key {
            n = n.key(k);
        }
        n
    }
}

// Overlays
mod dialog;
mod menu;
mod overlay_surface;
mod popover;
mod select;
mod toast;
mod tooltip;
pub use dialog::{AlertDialog, Dialog};
pub use menu::{ContextMenu, Menu, MenuItem};
pub use popover::Popover;
pub use select::Select;
pub use toast::{Toast, Toaster};
pub use tooltip::Tooltip;

// Layout and data
mod chart;
mod data;
mod feedback;
mod navigation;
mod stepper;
pub use chart::{Chart, ChartSeries, QuotaBar};
pub use data::{ScrollView, SortDirection, Table, Tree, TreeNode};
pub use feedback::{Avatar, Ring, Skeleton, Spinner};
pub use navigation::{Accordion, Disclosure, Sidebar, Tabs, Toolbar};
pub use stepper::Stepper;

// Text
mod combobox;
mod palette;
mod text;
pub use combobox::{Combobox, QueryState};
pub use palette::{Command, CommandPalette};
pub use text::{SearchField, SecureField, TextArea, TextField};
