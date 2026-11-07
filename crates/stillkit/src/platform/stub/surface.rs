use crate::{DisplayList, HitId, Point, Ui};
use anyhow::{Result, bail};
/// What kind of window a surface is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A topmost popup: menus, flyouts. With `activate` it takes the keyboard focus when
    /// asked ([`Surface::take_focus`]); without, clicking it never activates it (submenus).
    Popup { activate: bool },
    /// A floating bar: topmost and never activated. Its app-bar reservation
    /// (`SHAppBarMessage`) belongs to the bar (N2).
    Bar,
    /// Desktop-layer surface. The name is retained for callers, but this is a top-level
    /// layered tool window anchored immediately above `parent` (Progman), never a WS_CHILD.
    /// Raised desktops do not display ULW children, and WGC cannot exclude child windows.
    DesktopChild { parent: usize },
    /// A normal app window (Studio): resizable, activated, in the taskbar, drawn opaque.
    /// `min` is its smallest client size in design pixels. See [`crate::app_window`].
    App { min: (u32, u32) },
}

/// Identifies a surface in [`Event`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(isize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
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
    Move(Pointer),
    Leave,
    /// Positive delta scrolls up; position uses the same coordinates as other pointer events.
    Wheel(Pointer, i16),
    CaptureLost,
    Press(Pointer, Button),
    Release(Pointer, Button),
    DoubleClick(Pointer, Button),
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
    DpiChanged(u32),
    VisibilityChanged,
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

pub struct AnimationFrames;
pub fn next_event() -> Option<(SurfaceId, Event)> {
    unimplemented!("native event queue")
}
pub fn queue_event(_: usize, _: Event) {
    unimplemented!("native event queue")
}
pub struct Surface;
impl Surface {
    pub fn new(_: Kind, _: &str, _: accesskit::Role) -> Result<Self> {
        bail!("no native window backend")
    }
    pub fn new_deferred_accessibility(
        kind: Kind,
        name: &str,
        role: accesskit::Role,
    ) -> Result<Self> {
        Self::new(kind, name, role)
    }
    pub fn present(
        &mut self,
        _: &mut Ui,
        _: (i32, i32),
        _: (u32, u32),
        _: DisplayList,
    ) -> Result<bool> {
        bail!("no native window backend")
    }
    pub fn shown(&self) -> &DisplayList {
        unimplemented!("native window")
    }
    pub fn id(&self) -> SurfaceId {
        unimplemented!("native window")
    }
    pub fn origin(&self) -> (i32, i32) {
        unimplemented!("native window")
    }
    pub fn animation_frames(&self, _: std::time::Duration) -> AnimationFrames {
        unimplemented!("frame scheduling")
    }
    pub fn set_alpha(&mut self, _: u8) -> Result<()> {
        bail!("no native window backend")
    }
    pub fn move_to(&mut self, _: (i32, i32)) -> Result<()> {
        bail!("no native window backend")
    }
    pub fn set_accessibility_semantics(&self, _: Vec<(crate::HitId, crate::Semantics)>) {}
    pub fn take_accessibility_action(&self, _: crate::HitId) -> Option<crate::AccessibilityAction> {
        None
    }
    pub fn set_glass(&self, _: Option<crate::Glass>) {
        unimplemented!("glass")
    }
    pub fn set_glass_style(&self, _: crate::glass::GlassStyle) {
        unimplemented!("glass")
    }
    pub fn show(&self) {
        unimplemented!("native window")
    }
    pub fn hide(&self) {
        unimplemented!("native window")
    }
    pub fn release(&mut self) {
        unimplemented!("native window")
    }
    pub fn discard_events(&self) {
        unimplemented!("native event queue")
    }
    pub fn take_focus(&self) -> &'static str {
        unimplemented!("native focus")
    }
    // App-window services (`Kind::App`).
    pub fn scale(&self) -> f32 {
        unimplemented!("native window")
    }
    pub fn place(&self, _: (i32, i32, i32, i32)) {
        unimplemented!("native window")
    }
    pub fn client_size(&self) -> (u32, u32) {
        unimplemented!("native window")
    }
    pub fn show_active(&self, _: bool) {
        unimplemented!("native window")
    }
    pub fn set_chrome(&self, _: &[(crate::Rect, super::app_window::Chrome)]) {
        unimplemented!("native window")
    }
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
