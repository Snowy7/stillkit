//! CPU rendering and native Wayland surfaces, plus opt-in Vulkan rendering and glass.
mod a11y;
mod a11y_bus;
pub mod app_window;
/// Glass services.
pub mod glass;
#[cfg(feature = "gpu")]
pub mod gpu;
#[cfg(feature = "gpu")]
mod gpu_glass;
pub mod caret;
pub mod clipboard;
pub mod ime;
pub mod host;
mod keyboard;
pub mod render;
pub mod screen;
/// Surface services.
pub mod surface;
pub mod text;
pub mod text_field;
pub mod timing;
mod wake;
/// Layered app services.
pub mod layered_app {}
/// Compose services.
pub mod compose {}
pub mod modules;
/// Portable process identity.
pub mod identity {
    pub use crate::identity::{Identity, set_identity};
}
/// Watch services.
pub mod watch {
    /// File watch.
    pub struct FileWatch;
}
pub use render::{Image, Ui};
pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
pub use text::Text;
pub use text_field::TextField;
pub mod icons;
pub use render::Canvas as Renderer;
/// Selected native services (or compile-only stand-ins on unsupported platforms).
pub struct Backend;
impl super::Backend for Backend {
    type Renderer = Renderer;
    type Text = Text;
    type Window = Surface;
    type TextInput = TextField;
}
