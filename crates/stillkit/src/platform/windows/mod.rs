//! Windows CPU renderer and native window services.
pub mod a11y;
pub mod app_window;
pub mod compose;
pub(crate) mod dialog;
pub mod clipboard;
/// Glass services.
pub mod glass;
#[cfg(feature = "gpu")]
pub mod gpu;
#[cfg(feature = "gpu")]
pub mod gpu_glass;
pub mod caret;
pub mod host;
pub mod identity;
pub mod layered_app;
pub mod modules;
pub mod render;
/// Screen services.
pub mod screen;
pub mod surface;
pub mod system_backdrop;
pub mod text;
pub mod text_field;
/// Timing services.
pub mod timing;
pub mod watch;
pub use render::{Image, Ui};
pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
pub use text::Text;
pub use text_field::TextField;
/// Icons services.
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
