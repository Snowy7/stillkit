//! Compile-only backend. Native services are unavailable until a backend is installed.
pub mod glass;
pub mod host;
pub mod render;
pub mod screen;
pub mod surface;
pub mod text;
pub mod text_field;
pub mod timing;
pub mod app_window {
    /// Window chrome parts an app window hit-tests (see the Windows backend).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Chrome {
        Caption,
        Minimize,
        Maximize,
        Close,
    }
    pub enum Cursor {}
    pub fn set_modal_hook(_: Option<fn(bool)>) {
        unimplemented!("native windows")
    }
}
pub mod layered_app {}
pub mod compose {}
pub mod modules {}
pub mod identity {
    pub struct Identity;
    pub fn set_identity(_: Identity) {
        unimplemented!("application identity")
    }
}
pub mod watch {
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
