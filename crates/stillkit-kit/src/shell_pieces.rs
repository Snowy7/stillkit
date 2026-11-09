//! Small app-shell building blocks with app-owned state and runtime-owned motion.
use crate::*;
use std::time::Duration;
use stillkit::el::{KeyCode, Modifiers};
use stillkit::{Event, Rect};

/// A shortcut rendered as individual keycaps.
pub struct Keybind(Vec<Arc<str>>);
impl Keybind {
    /// Labels such as `["Ctrl", "Shift", "K"]`; no shortcut is registered.
    pub fn new(keys: impl IntoIterator<Item = impl Into<Arc<str>>>) -> Self {
        Self(keys.into_iter().map(Into::into).collect())
    }
    /// Build a labelled group of theme-shaped keycaps.
    pub fn build<S>(self) -> El<S> {
        let label = self
            .0
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join(" + ");
        self.0.into_iter().enumerate().fold(
            row()
                .gap(3.)
                .align(Align::Center)
                .role(Role::Group)
                .label(label),
            |r, (i, key)| {
                r.child(
                    text(key)
                        .key(format!("key-{i}"))
                        .font(TextRole::Caption)
                        .color(LABEL)
                        .pad_x(5.)
                        .pad_y(2.)
                        .radius(token::RADIUS_CONTROL)
                        .fill(token::FILL),
                )
            },
        )
    }
}

/// One virtual-key chord. The app owns validation and registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyChord {
    /// Main virtual key, using the same codes as `Event::Key`.
    pub key: KeyCode,
    /// Modifiers held when the main key went down.
    pub modifiers: Modifiers,
}
impl KeyChord {
    /// Human-readable modifier names followed by the key.
    pub fn keycaps(self) -> Vec<String> {
        let mut keys = modifier_caps(self.modifiers);
        keys.push(match self.key {
            KeyCode::SPACE => "Space".into(),
            KeyCode::TAB => "Tab".into(),
            KeyCode::ENTER => "Enter".into(),
            KeyCode::LEFT => "Left".into(),
            KeyCode::RIGHT => "Right".into(),
            KeyCode::UP => "Up".into(),
            KeyCode::DOWN => "Down".into(),
            KeyCode(vk @ 0x70..=0x87) => format!("F{}", vk - 0x6f),
            KeyCode(vk @ (0x30..=0x39 | 0x41..=0x5a)) => {
                char::from_u32(vk as u32).unwrap().to_string()
            }
            KeyCode(vk) => format!("Key {vk:02X}"),
        });
        keys
    }
}
fn modifier_caps(m: Modifiers) -> Vec<String> {
    [
        (m.ctrl, "Ctrl"),
        (m.alt, "Alt"),
        (m.shift, "Shift"),
        (m.meta, "Super"),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .map(|(_, name)| name.to_owned())
    .collect()
}
fn modifier(vk: u16) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5b..=0x5c | 0xa0..=0xa5)
}

/// Recording state. Forward host events through `handle` while recording.
#[derive(Clone, Debug, Default)]
pub struct ShortcutRecorderState {
    /// The accepted chord, suitable for app settings.
    pub chord: Option<KeyChord>,
    /// Whether this control is currently recording.
    pub recording: bool,
    held: Modifiers,
}
impl ShortcutRecorderState {
    /// Begin a new recording, preserving the old chord until acceptance.
    pub fn begin(&mut self) {
        self.recording = true;
        self.held = Modifiers::NONE;
    }
    /// Forward both Key and KeyUp before Runtime::handle; true means consumed.
    /// `mods` is the host's current modifier snapshot. Deactivation cancels recording.
    pub fn handle(&mut self, event: &Event, mods: Modifiers) -> bool {
        if !self.recording {
            return false;
        }
        match *event {
            Event::KeyUp(_) => {
                self.held = mods;
                true
            }
            Event::Deactivated => {
                self.recording = false;
                self.held = Modifiers::NONE;
                false
            }
            Event::Key(vk) => {
                self.held = mods;
                if modifier(vk) {
                    return true;
                }
                match KeyCode(vk) {
                    KeyCode::ESCAPE => self.recording = false,
                    KeyCode::BACKSPACE | KeyCode::DELETE => {
                        self.chord = None;
                        self.recording = false;
                    }
                    key => {
                        self.chord = Some(KeyChord {
                            key,
                            modifiers: mods,
                        });
                        self.recording = false;
                    }
                }
                true
            }
            _ => false,
        }
    }
}
/// Activate to record; the host forwards key-up events to its state.
pub struct ShortcutRecorder<S> {
    key: Arc<str>,
    label: Arc<str>,
    state: ShortcutRecorderState,
    access: fn(&mut S) -> &mut ShortcutRecorderState,
}
impl<S: 'static> ShortcutRecorder<S> {
    /// Give the accessible label the action's name, for example "Open palette".
    pub fn new(
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        state: &ShortcutRecorderState,
        access: fn(&mut S) -> &mut ShortcutRecorderState,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            state: state.clone(),
            access,
        }
    }
    /// Build the recorder; Escape cancels and Backspace clears while recording.
    pub fn build(self) -> El<S> {
        let keys = if self.state.recording {
            modifier_caps(self.state.held)
        } else {
            self.state.chord.map_or_else(Vec::new, KeyChord::keycaps)
        };
        let hint = if self.state.recording {
            "Press shortcut"
        } else {
            "Record shortcut"
        };
        let value = if keys.is_empty() {
            hint.to_owned()
        } else {
            keys.join(" + ")
        };
        let access = self.access;
        row()
            .key(self.key)
            .role(Role::Button)
            .label(self.label)
            .value(value)
            .description("Activate to record. Escape cancels; Backspace clears.")
            .pad_x(10.)
            .h(token::CONTROL)
            .align(Align::Center)
            .radius(token::RADIUS_CONTROL)
            .fill(token::WELL)
            .on_press(move |s| access(s).begin())
            .child(if keys.is_empty() {
                text(hint).color(SECONDARY)
            } else {
                Keybind::new(keys).build()
            })
    }
}

/// Delayed hover state. Feed pointer events with the current anchor and card bounds.
#[derive(Clone, Debug, Default)]
pub struct HoverCardState {
    /// Whether rich content is visible.
    pub open: bool,
    deadline: Option<Duration>,
}
impl HoverCardState {
    /// Process window input in design pixels, preserving hover while inside either region.
    /// Returns whether the app view needs rebuilding. Keyboard focus may call `enter` too.
    pub fn handle(
        &mut self,
        event: &Event,
        anchor: Rect,
        card: Option<Rect>,
        scale: f32,
        now: Duration,
    ) -> bool {
        match *event {
            Event::Move(p) => {
                let x = p.at.x / scale;
                let y = p.at.y / scale;
                let inside = |r: Rect| {
                    x >= r.left - 8. && x <= r.right + 8. && y >= r.top - 8. && y <= r.bottom + 8.
                };
                if inside(anchor) || self.open && card.is_some_and(inside) {
                    self.enter(now);
                } else {
                    return self.dismiss();
                }
            }
            Event::Leave | Event::Deactivated | Event::Key(0x1b) => return self.dismiss(),
            _ => {}
        }
        false
    }
    /// Arm a single delay after pointer entry or keyboard focus.
    pub fn enter(&mut self, now: Duration) {
        if !self.open && self.deadline.is_none() {
            self.deadline = Some(now + Duration::from_millis(500));
        }
    }
    /// Host wake deadline; no repeating timer is needed.
    pub fn deadline(&self) -> Option<Duration> {
        self.deadline
    }
    /// Open once at the deadline; returns whether the view changed.
    pub fn advance(&mut self, now: Duration) -> bool {
        if self.deadline.is_some_and(|d| now >= d) {
            self.deadline = None;
            self.open = true;
            true
        } else {
            false
        }
    }
    /// Close and cancel the pending delay.
    pub fn dismiss(&mut self) -> bool {
        let changed = self.open;
        self.open = false;
        self.deadline = None;
        changed
    }
}
/// Rich interactive content in a non-modal popover, with host-owned hover tracking.
pub struct HoverCard<S> {
    key: Option<Arc<str>>,
    content: El<S>,
    open: bool,
    dismiss: Press<S>,
}
impl<S: 'static> HoverCard<S> {
    /// Attach `overlay()` to the trigger, and track both bounds using `HoverCardState`.
    pub fn new(content: El<S>, open: bool, dismiss: impl Fn(&mut S) + 'static) -> Self {
        Self {
            key: None,
            content,
            open,
            dismiss: Box::new(dismiss),
        }
    }
    /// Name the complete panel so host hover tracking includes its padding.
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Interactive controls remain in the keyboard order; Escape dismisses.
    pub fn overlay(self) -> stillkit::el::Overlay<S> {
        let mut panel = Popover::new(self.content).build();
        if let Some(key) = self.key {
            panel = panel.key(key);
        }
        stillkit::el::popover(panel)
            .open(self.open)
            .offset(4.)
            .on_dismiss(self.dismiss)
    }
}

/// Replace a complete text run with a short slide and fade. Unicode stays shaped together.
pub struct RollingText {
    key: Arc<str>,
    value: Arc<str>,
}
impl RollingText {
    /// Stable label identity and current formatted number or text.
    pub fn new(key: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
    /// Runtime stops animation when settled and honors reduced motion.
    pub fn build<S>(self) -> El<S> {
        row().key(self.key).motion(token::MOTION_INTERACTIVE).child(
            text(self.value.clone())
                .key(self.value)
                .color(LABEL)
                .transition(Transition::slide(0., 8.)),
        )
    }
}

/// A sidebar heading with a count and app-controlled expansion.
pub struct SidebarHeader<S> {
    title: Arc<str>,
    count: usize,
    open: bool,
    toggle: Press<S>,
}
impl<S: 'static> SidebarHeader<S> {
    /// Count describes all rows, including those hidden by collapse.
    pub fn new(
        title: impl Into<Arc<str>>,
        count: usize,
        open: bool,
        toggle: impl Fn(&mut S) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            count,
            open,
            toggle: Box::new(toggle),
        }
    }
    /// Build the keyboard-activatable disclosure header.
    pub fn build(self) -> El<S> {
        let toggle: Arc<dyn Fn(&mut S)> = self.toggle.into();
        let press = toggle.clone();
        let open = self.open;
        row()
            .h(token::CONTROL_SMALL)
            .pad_x(10.)
            .gap(token::ITEM)
            .align(Align::Center)
            .role(Role::Button)
            .label(format!("{}, {} items", self.title, self.count))
            .expanded(self.open)
            .on_press(move |s| press(s))
            .on_key(move |s, key, _| {
                if key == KeyCode::LEFT && open || key == KeyCode::RIGHT && !open {
                    toggle(s);
                    true.into()
                } else {
                    false.into()
                }
            })
            .child(
                icon(Icon::ChevronRight)
                    .size(token::ICON)
                    .color(SECONDARY)
                    .rotate(if self.open {
                        std::f32::consts::FRAC_PI_2
                    } else {
                        0.
                    })
                    .animate(),
            )
            .child(
                text(self.title)
                    .grow(1.)
                    .font(TextRole::Subheadline)
                    .color(SECONDARY),
            )
            .child(
                text(self.count.to_string())
                    .font(TextRole::Caption)
                    .color(SECONDARY),
            )
    }
}
/// A labelled collapsible group; hidden rows leave focus and accessibility trees.
pub struct SidebarSection<S> {
    header: SidebarHeader<S>,
    children: Vec<El<S>>,
}
impl<S: 'static> SidebarSection<S> {
    /// Supply the model's open state and toggle callback in the header.
    pub fn new(header: SidebarHeader<S>) -> Self {
        Self {
            header,
            children: Vec::new(),
        }
    }
    /// Add a row or other sidebar content.
    pub fn child(mut self, child: El<S>) -> Self {
        self.children.push(child);
        self
    }
    /// Build the header and, when expanded, its rows.
    pub fn build(self) -> El<S> {
        let open = self.header.open;
        let root = col().gap(2.).child(self.header.build());
        if open {
            self.children.into_iter().fold(root, |r, c| r.child(c))
        } else {
            root
        }
    }
}
/// Selectable source-list row with a leading icon and optional count.
pub struct SidebarRow<S> {
    title: Arc<str>,
    glyph: Icon,
    selected: bool,
    count: Option<usize>,
    press: Press<S>,
}
impl<S: 'static> SidebarRow<S> {
    /// Selection is app-owned; arrows work when rows are inside a vertical focus group.
    pub fn new(
        title: impl Into<Arc<str>>,
        glyph: Icon,
        selected: bool,
        press: impl Fn(&mut S) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            glyph,
            selected,
            count: None,
            press: Box::new(press),
        }
    }
    /// Show a trailing item count.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }
    /// Build a selected list item with standard kit interaction feedback.
    pub fn build(self) -> El<S> {
        let mut r = row()
            .h(token::CONTROL_SMALL)
            .pad_x(10.)
            .gap(token::ITEM)
            .align(Align::Center)
            .radius(token::RADIUS_CONTROL)
            .role(Role::ListItem)
            .label(self.title.clone())
            .selected(self.selected)
            .fill(if self.selected {
                token::FILL
            } else {
                Paint::Rgba(Rgba::TRANSPARENT)
            })
            .on_press(self.press)
            .child(icon(self.glyph).size(token::ICON).color(SECONDARY))
            .child(text(self.title).grow(1.).color(LABEL));
        if let Some(n) = self.count {
            r = r.child(text(n.to_string()).color(SECONDARY).font(TextRole::Caption));
        }
        r
    }
}
/// Title, optional toolbar and flexible content scaffold.
pub struct Page<S> {
    title: Arc<str>,
    toolbar: Option<El<S>>,
    content: El<S>,
}
impl<S: 'static> Page<S> {
    /// The title is announced as a heading.
    pub fn new(title: impl Into<Arc<str>>, content: El<S>) -> Self {
        Self {
            title: title.into(),
            toolbar: None,
            content,
        }
    }
    /// Add search, menus or page actions beside the title.
    pub fn toolbar(mut self, toolbar: El<S>) -> Self {
        self.toolbar = Some(toolbar);
        self
    }
    /// Build the full-size page using theme padding and spacing.
    pub fn build(self) -> El<S> {
        let mut header = row().gap(token::GAP).align(Align::Center).child(
            text(self.title.clone())
                .grow(1.)
                .font(TextRole::Title2)
                .role(Role::Heading)
                .label(self.title)
                .color(LABEL),
        );
        if let Some(t) = self.toolbar {
            header = header.child(t);
        }
        col()
            .w_full()
            .h_full()
            .pad(token::PAD)
            .gap(token::GAP)
            .child(header)
            .child(col().min_h(0.).min_w(0.).grow(1.).child(self.content))
    }
}
