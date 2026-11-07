//! macOS: AppKit windows presenting a CoreGraphics bitmap as their layer's contents, CoreText
//! text, the AppKit event loop (asleep in `nextEventMatchingMask` while idle), NSTextInputClient
//! input, NSPasteboard and AccessKit's NSAccessibility adapter; Metal (wgpu, Vello Hybrid and
//! in-process glass) only with the `gpu` feature. docs/ui.md, "macOS backend".
//!
//! Compile-checked on Windows; the "to test on the Mac" list in docs/ui.md is what has not run.
mod a11y;
pub mod app_window;
pub mod caret;
pub mod clipboard;
mod draw;
pub mod glass;
#[cfg(feature = "gpu")]
pub mod gpu;
#[cfg(feature = "gpu")]
#[path = "../linux/gpu_glass.rs"]
mod gpu_glass;
#[cfg(feature = "gpu")]
mod gpu_scene;
pub mod host;
pub mod icons;
pub mod ime;
mod keys;
pub mod modules;
mod pump;
pub mod render;
pub mod screen;
pub mod surface;
pub mod text;
pub mod text_field;
pub mod timing;
mod view;
pub mod layered_app {}
pub mod compose {}
/// Portable process identity.
pub mod identity {
    pub use crate::identity::{Identity, set_identity};
}
pub mod watch {
    pub struct FileWatch;
}
pub use render::{Image, Ui};
pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
pub use text::Text;
pub use text_field::TextField;
pub use render::Canvas as Renderer;
/// Selected native services.
pub struct Backend;
impl super::Backend for Backend {
    type Renderer = Renderer;
    type Text = Text;
    type Window = Surface;
    type TextInput = TextField;
}
