use crate::{DisplayList, HitId, Point, Ui};
use anyhow::{Result, bail};
/// What kind of window a surface is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A topmost popup: menus, flyouts. With `activate` it takes the keyboard focus when
    /// asked ([`Surface::take_focus`]); without, clicking it never activates it (submenus).
    Popup { #[doc = "Whether activation is requested."] activate: bool },
    /// A floating bar: topmost and never activated. Its app-bar reservation
    /// (`SHAppBarMessage`) belongs to the bar (N2).
    Bar,
    /// Desktop-layer surface. The name is retained for callers, but this is a top-level
    /// layered tool window anchored immediately above `parent` (Progman), never a WS_CHILD.
    /// Raised desktops do not display ULW children, and WGC cannot exclude child windows.
    DesktopChild { #[doc = "Native desktop parent handle."] parent: usize },
    /// A normal app window (Studio): resizable, activated, in the taskbar, drawn opaque.
    /// `min` is its smallest client size in design pixels. See [`crate::app_window`].
    App { #[doc = "Minimum client size in design pixels."] min: (u32, u32) },
}

/// Identifies a surface in [`Event`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(isize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Button.
pub enum Button {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Middle.
    Middle,
}

/// Where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// In the surface's pixels.
    pub at: Point,
    /// In screen pixels.
    pub screen: (i32, i32),
}

/// Input for a surface, taken from [`next_event`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// A temporary animation requested another frame; no timer remains once it settles.
    Frame,
    /// Move.
    Move(Pointer),
    /// Leave.
    Leave,
    /// Positive delta scrolls up; position uses the same coordinates as other pointer events.
    Wheel(Pointer, i16),
    /// Capture lost.
    CaptureLost,
    /// Press.
    Press(Pointer, Button),
    /// Release.
    Release(Pointer, Button),
    /// Double click.
    DoubleClick(Pointer, Button),
    /// Files dragged from another app are over the surface ([`take_dropped_files`] after
    /// [`Event::FilesDropped`]).
    FilesHover(Pointer),
    /// The dragged files left the surface, or the drag was cancelled.
    FilesLeave,
    /// Files were dropped here; [`take_dropped_files`] has their paths.
    FilesDropped(Pointer),
    /// A key went down (a virtual-key code).
    Key(u16),
    /// A desktop surface activated; Explorer may finish reordering its band afterwards.
    Activated,
    /// The surface lost the activation (another window was clicked or focused).
    Deactivated,
    /// Assistive technology invoked a hit item.
    Invoke(HitId),
    /// Assistive technology moved the focus to a hit item.
    Focus(HitId),
    /// Dpi changed.
    DpiChanged(u32),
    /// Visibility changed.
    VisibilityChanged,
    /// Cancelled.
    Cancelled,
    /// Windowless RichEdit changed text, selection or caret.
    TextChanged,
    /// Enter in a single-line windowless field.
    TextSubmit,
    /// An app window's client area is now this many pixels.
    Resized(u32, u32),
    /// The user asked an app window to close (its close button, Alt+F4, the taskbar).
    Close,
    /// Windows' settings or theme changed (light or dark, contrast, text size).
    SettingChanged,
}

/// Animation frames.
pub struct AnimationFrames;
/// Next event.
pub fn next_event() -> Option<(SurfaceId, Event)> {
    unimplemented!("native event queue")
}
/// Queue event.
pub fn queue_event(_: usize, _: Event) {
    unimplemented!("native event queue")
}
/// Surface.
pub struct Surface;
impl Surface {
    /// Creates a new instance.
    pub fn new(_: Kind, _: &str, _: accesskit::Role) -> Result<Self> {
        bail!("no native window backend")
    }
    /// New deferred accessibility.
    pub fn new_deferred_accessibility(
        kind: Kind,
        name: &str,
        role: accesskit::Role,
    ) -> Result<Self> {
        Self::new(kind, name, role)
    }
    /// Present.
    pub fn present(
        &mut self,
        _: &mut Ui,
        _: (i32, i32),
        _: (u32, u32),
        _: DisplayList,
    ) -> Result<bool> {
        bail!("no native window backend")
    }
    /// Shown.
    pub fn shown(&self) -> &DisplayList {
        unimplemented!("native window")
    }
    /// Stable identity used to match this item across frames.
    pub fn id(&self) -> SurfaceId {
        unimplemented!("native window")
    }
    /// Origin.
    pub fn origin(&self) -> (i32, i32) {
        unimplemented!("native window")
    }
    /// Animation frames.
    pub fn animation_frames(&self, _: std::time::Duration) -> AnimationFrames {
        unimplemented!("frame scheduling")
    }
    /// Sets alpha.
    pub fn set_alpha(&mut self, _: u8) -> Result<()> {
        bail!("no native window backend")
    }
    /// Move to.
    pub fn move_to(&mut self, _: (i32, i32)) -> Result<()> {
        bail!("no native window backend")
    }
    /// Sets accessibility semantics.
    pub fn set_accessibility_semantics(&self, _: Vec<(crate::HitId, crate::Semantics)>) {}
    /// Takes accessibility action from the pending state.
    pub fn take_accessibility_action(&self, _: crate::HitId) -> Option<crate::AccessibilityAction> {
        None
    }
    /// Sets glass.
    pub fn set_glass(&self, _: Option<crate::Glass>) {
        unimplemented!("glass")
    }
    /// Sets glass style.
    pub fn set_glass_style(&self, _: crate::glass::GlassStyle) {
        unimplemented!("glass")
    }
    /// GPU mode is Windows-only: every mode draws on the CPU here.
    pub fn with_render(kind: Kind, name: &str, role: accesskit::Role, _: crate::platform::Render) -> Result<Self> {
        Self::new(kind, name, role)
    }
    /// Render.
    pub fn render(&self) -> crate::platform::Render {
        crate::platform::Render::Cpu
    }
    /// Glass available.
    pub fn glass_available(&self) -> bool {
        crate::glass::available()
    }
    /// Sets glass group.
    pub fn set_glass_group(&self, glasses: &[crate::Glass], _spacing: f32) {
        self.set_glass(glasses.first().copied())
    }
    /// Show.
    pub fn show(&self) {
        unimplemented!("native window")
    }
    /// Hide.
    pub fn hide(&self) {
        unimplemented!("native window")
    }
    /// Release.
    pub fn release(&mut self) {
        unimplemented!("native window")
    }
    /// Discard events.
    pub fn discard_events(&self) {
        unimplemented!("native event queue")
    }
    /// Takes focus from the pending state.
    pub fn take_focus(&self) -> &'static str {
        unimplemented!("native focus")
    }
    // App-window services (`Kind::App`).
    /// Scale.
    pub fn scale(&self) -> f32 {
        unimplemented!("native window")
    }
    /// Place.
    pub fn place(&self, _: (i32, i32, i32, i32)) {
        unimplemented!("native window")
    }
    /// Client size.
    pub fn client_size(&self) -> (u32, u32) {
        unimplemented!("native window")
    }
    /// Show active.
    pub fn show_active(&self, _: bool) {
        unimplemented!("native window")
    }
    /// Sets chrome.
    pub fn set_chrome(&self, _: &[(crate::Rect, super::app_window::Chrome)]) {
        unimplemented!("native window")
    }
    /// Sets background.
    pub fn set_background(&self, _: crate::Rgba) {
        unimplemented!("native window")
    }
}
impl crate::platform::Window for Surface {
    type Bitmap = ();
    fn new(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
        Self::new(kind, name, role)
    }
    fn present_bitmap(
        &mut self,
        _: (i32, i32),
        _: Option<(&(), crate::PixelRect, bool)>,
    ) -> Result<()> {
        bail!("no native window backend")
    }
    fn next_event() -> Option<(SurfaceId, Event)> {
        next_event()
    }
}

thread_local! {
    static DROPPED: std::cell::RefCell<Vec<std::path::PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The paths of the files dropped with the last [`Event::FilesDropped`] (taken once).
pub fn take_dropped_files() -> Vec<std::path::PathBuf> {
    DROPPED.with_borrow_mut(std::mem::take)
}

/// Keeps dropped paths for [`take_dropped_files`].
#[allow(dead_code)]
pub(super) fn set_dropped_files(paths: Vec<std::path::PathBuf>) {
    DROPPED.with_borrow_mut(|d| *d = paths);
}
