//! Compile-only backend. Native services are unavailable until a backend is installed.
/// Glass.
pub mod glass;
/// Caret.
pub mod caret;
/// Host.
pub mod host;
/// Render.
pub mod render;
/// Screen.
pub mod screen;
/// Surface.
pub mod surface;
/// Text content.
pub mod text;
/// Text field.
pub mod text_field;
/// Timing.
pub mod timing;
/// App window.
pub mod app_window {
    /// Window chrome parts an app window hit-tests (see the Windows backend).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Chrome {
        /// Caption.
        Caption,
        /// Minimize.
        Minimize,
        /// Maximize.
        Maximize,
        /// Close.
        Close,
    }
    /// Cursor.
    pub enum Cursor {}
    /// Sets modal hook.
    pub fn set_modal_hook(_: Option<fn(bool)>) {
        unimplemented!("native windows")
    }
}
/// Layered app.
pub mod layered_app {}
/// Compose.
pub mod compose {}
/// Modules.
pub mod modules {}
/// Portable process identity.
pub mod identity {
    pub use crate::identity::{Identity, set_identity};
}
/// Watch.
pub mod watch {
    /// File watch.
    pub struct FileWatch;
}
pub use render::{Image, Ui};
pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
pub use text::Text;
pub use text_field::TextField;
/// Icons.
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
