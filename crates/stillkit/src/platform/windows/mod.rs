//! Windows CPU renderer and native window services.
pub mod a11y;
pub mod app_window;
pub mod compose;
pub mod glass;
pub mod host;
pub mod identity;
pub mod layered_app;
pub mod modules;
pub mod render;
pub mod screen;
pub mod surface;
pub mod text;
pub mod text_field;
pub mod timing;
pub mod watch;
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
