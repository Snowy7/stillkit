//! Static backend boundary. Core selects concrete types through `current`.
#[cfg(not(windows))]
pub mod stub;
#[cfg(windows)]
pub mod windows;
use crate::{DisplayList, PixelRect, TextStyle};
use anyhow::Result;
#[cfg(not(windows))]
pub use stub as current;
#[cfg(windows)]
pub use windows as current;
/// CPU bitmap target: draw only the requested damage, then read its pixels.
pub trait Renderer: Sized {
    type Context;
    fn new(context: &Self::Context, width: u32, height: u32) -> Result<Self>;
    fn draw(&self, text: &mut current::Text, list: &DisplayList, damage: PixelRect) -> Result<()>;
    fn read(&self, damage: PixelRect, sink: impl FnMut(u32, &[u8])) -> Result<()>;
}
/// Native text shaping and per-thread format/layout caching.
pub trait TextSystem {
    type Format;
    type Layout;
    fn format(&mut self, style: &TextStyle) -> Option<Self::Format>;
    fn layout(&mut self, text: &str, style: &TextStyle) -> Option<&Self::Layout>;
    fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32);
    fn clear_cache(&mut self);
}
/// Native window kinds, bitmap presentation and queued input.
pub trait Window: Sized {
    type Bitmap;
    fn new(kind: current::Kind, name: &str, role: accesskit::Role) -> Result<Self>;
    /// Presents the native bitmap's dirty rect (and resize flag), or moves without repainting.
    fn present_bitmap(
        &mut self,
        origin: (i32, i32),
        bitmap: Option<(&Self::Bitmap, PixelRect, bool)>,
    ) -> Result<()>;
    fn next_event() -> Option<(current::SurfaceId, current::Event)>;
}
/// Windowless native text editing/IME; the backend owns focus and composition.
pub trait TextInput {
    fn text(&self) -> Result<String>;
    fn set_text(&self, text: &str) -> Result<()>;
    fn focus(&self);
    fn blur(&self);
    fn caret(&self) -> Option<crate::Rect>;
}

/// Associated concrete services of a backend; no virtual dispatch is involved.
pub trait Backend {
    type Renderer: Renderer;
    type Text: TextSystem;
    type Window: Window;
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
