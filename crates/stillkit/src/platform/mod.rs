//! Static backend boundary. Core selects concrete types through `current`.
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux as current;
// The AT-SPI status codec is pure bytes, so its tests run on every host.
#[cfg(all(test, not(target_os = "linux")))]
#[path = "linux/a11y_bus.rs"]
mod a11y_bus;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos as current;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub mod stub;
#[cfg(windows)]
pub mod windows;
use crate::{DisplayList, PixelRect, TextStyle};
use anyhow::Result;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub use stub as current;
#[cfg(windows)]
pub use windows as current;
/// CPU bitmap target: draw only the requested damage, then read its pixels.
pub trait Renderer: Sized {
    /// Native resources needed to create this renderer.
    type Context;
    /// Creates a new instance.
    fn new(context: &Self::Context, width: u32, height: u32) -> Result<Self>;
    /// Draws into the current display list.
    fn draw(&self, text: &mut current::Text, list: &DisplayList, damage: PixelRect) -> Result<()>;
    /// Reads the requested pixels into the supplied sink.
    fn read(&self, damage: PixelRect, sink: impl FnMut(u32, &[u8])) -> Result<()>;
}
/// Native text shaping and per-thread format/layout caching.
pub trait TextSystem {
    /// Native text format type.
    type Format;
    /// Native shaped text layout type.
    type Layout;
    /// Creates a native text format for the supplied style.
    fn format(&mut self, style: &TextStyle) -> Option<Self::Format>;
    /// Layout.
    fn layout(&mut self, text: &str, style: &TextStyle) -> Option<&Self::Layout>;
    /// Measures the text in device pixels.
    fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32);
    /// Releases cached native text layouts.
    fn clear_cache(&mut self);
}
/// Native window kinds, bitmap presentation and queued input.
pub trait Window: Sized {
    /// Native bitmap used for presentation.
    type Bitmap;
    /// Creates a new instance.
    fn new(kind: current::Kind, name: &str, role: accesskit::Role) -> Result<Self>;
    /// Presents the native bitmap's dirty rect (and resize flag), or moves without repainting.
    fn present_bitmap(
        &mut self,
        origin: (i32, i32),
        bitmap: Option<(&Self::Bitmap, PixelRect, bool)>,
    ) -> Result<()>;
    /// Next event.
    fn next_event() -> Option<(current::SurfaceId, current::Event)>;
}
/// Windowless native text editing/IME; the backend owns focus and composition.
pub trait TextInput {
    /// Text content.
    fn text(&self) -> Result<String>;
    /// Sets text.
    fn set_text(&self, text: &str) -> Result<()>;
    /// The hit with keyboard focus.
    fn focus(&self);
    /// Blur.
    fn blur(&self);
    /// Caret.
    fn caret(&self) -> Option<crate::Rect>;
}

/// How a surface draws (docs/ui.md, "Render modes"). The app sets the default for its UI thread
/// with [`set_render`] before it makes surfaces; a surface can choose its own
/// (`Surface::with_render`). CPU and GPU surfaces mix freely on one thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Render {
    /// Software Direct2D on a WIC bitmap, in a per-pixel-alpha layered window: no GPU device in
    /// the process. Glass, if any, comes from a [`Backdrop`](crate::glass::Backdrop): an
    /// installed one, else (Windows) the system backdrop's frost, drawn by DWM.
    #[default]
    Cpu,
    /// Hardware Direct2D on a DirectComposition surface, with in-process Liquid Glass on the
    /// same adapter on Windows; Vello Hybrid on Vulkan (Linux) or Metal (macOS). Needs the `gpu` feature;
    /// unsupported platforms fall back to `Cpu`.
    Gpu,
    /// `Cpu` unless the surface wants glass ([`crate::glass::set_style`] holds a material) that
    /// no backdrop draws in full on the CPU ([`crate::glass::cpu_backdrop`]: none is installed,
    /// and the system backdrop is unavailable or the material is lensed); then `Gpu` (if the
    /// `gpu` feature is on).
    Auto,
}

impl Render {
    /// The mode a new surface uses: `Cpu` or `Gpu`, never `Auto`. `cpu_backdrop` says a
    /// backdrop draws the wanted glass for a CPU surface ([`crate::glass::cpu_backdrop`]).
    pub fn resolve(self, wants_glass: bool, cpu_backdrop: bool) -> Render {
        let gpu = cfg!(all(any(windows, target_os = "linux", target_os = "macos"), feature = "gpu"));
        match self {
            Render::Gpu if gpu => Render::Gpu,
            Render::Auto if gpu && wants_glass && !cpu_backdrop => Render::Gpu,
            _ => Render::Cpu,
        }
    }
}

thread_local! { static RENDER: std::cell::Cell<Option<Render>> = const { std::cell::Cell::new(None) }; }

/// Sets this UI thread's default render mode for surfaces made from now on.
pub fn set_render(render: Render) {
    RENDER.set(Some(render));
}

/// This UI thread's default render mode: what [`set_render`] set, else `STILLKIT_RENDER`
/// (`cpu`, `gpu` or `auto`, read once per thread), else `Cpu`. Headless renders
/// (`render::render`) follow it too, so a GPU app's snapshots match its screen.
pub fn render() -> Render {
    RENDER.get().unwrap_or_else(|| {
        let render = match std::env::var("STILLKIT_RENDER").as_deref() {
            Ok("gpu") => Render::Gpu,
            Ok("auto") => Render::Auto,
            _ => Render::Cpu,
        };
        RENDER.set(Some(render));
        render
    })
}

/// Associated concrete services of a backend; no virtual dispatch is involved.
pub trait Backend {
    /// Renderer.
    type Renderer: Renderer;
    /// Text.
    type Text: TextSystem;
    /// Window.
    type Window: Window;
    /// Text input.
    type TextInput: TextInput;
}

/// Existing `Event::Key` codes, mapped by native backends before reaching core.
pub(crate) mod keys {
    pub const DOWN: u16 = 0x28;
    pub const UP: u16 = 0x26;
    pub const LEFT: u16 = 0x25;
    pub const RIGHT: u16 = 0x27;
    pub const ESCAPE: u16 = 0x1b;
    pub const RETURN: u16 = 0x0d;
}

/// How tall an app window's title area is where the system draws the window controls
/// ([`Surface::set_caption`](crate::Surface::set_caption); macOS: the traffic lights).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Caption {
    /// The system's standard title bar (32 design px on macOS 26+).
    #[default]
    Standard,
    /// A taller, compact-toolbar title area (40 design px on macOS 26+), with the controls
    /// centred in it: for an app's own title bar holding menus and tabs.
    Compact,
}
