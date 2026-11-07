//! Windows CPU renderer and native window services.
pub mod surface;
pub mod app_window;
pub mod layered_app;
pub mod render;
pub mod text;
pub mod text_field;
pub mod compose;
pub mod modules;
pub mod watch;
pub mod identity;
pub mod a11y;
pub mod glass;
pub mod screen;
pub mod timing;
pub use render::{Ui, Image};
pub use text::Text;
pub use surface::{Surface, Kind, Event, SurfaceId, next_event};
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
