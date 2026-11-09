//! The declarative element tree (docs/el.md): a view is `fn(&State) -> El<State>`, laid out
//! with flexbox (Taffy) and painted into today's [`DisplayList`](crate::DisplayList), so a
//! surface still redraws only the controls that changed.
//!
//! ```no_run
//! use stillkit::el::{self, App, El, col, text, token};
//! use stillkit::{Kind, Scheme, Style, TextRole, Theme};
//!
//! struct Hello { on: bool }
//!
//! fn view(s: &Hello) -> El<Hello> {
//!     col().pad(token::PAD).gap(token::GAP)
//!         .child(text(if s.on { "On" } else { "Off" }).font(TextRole::Title2).key("label"))
//!         .child(col().key("toggle").w(40.0).h(24.0).radius(12.0).fill(token::FILL)
//!             .checked(s.on).label("Power")
//!             .on_press(|s: &mut Hello| s.on = !s.on))
//! }
//!
//! el::run(App {
//!     title: "Hello",
//!     size: (240.0, 120.0),
//!     kind: Kind::App { min: (160, 100) },
//!     theme: Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
//!     state: Hello { on: false },
//!     view,
//! })
//! # .unwrap();
//! ```
//!
//! - [`El`] builders size, style and describe an element and attach handlers.
//! - [`Runtime`] holds the state and the retained tree; handlers run between frames, and only
//!   state changes re-run the view (hover, press and focus repaint the retained tree).
//! - [`run`] hosts a runtime in a window, and doubles as the agent tooling (`--snap`,
//!   `--tree`, `--script`).
//! - [`El::overlay`] with [`popover`], [`menu`], [`dialog`] or [`toasts`], and
//!   [`El::tooltip`], put anchored content above the window's (in a popup surface when it
//!   doesn't fit).
//! - [`text_input`] and [`text_area`] are text fields: the view owns the string
//!   ([`El::value`], [`El::on_input`]); caret moves, selection and the caret's blink never run
//!   the view. On Windows the focused one types through RichEdit (the system IME).

mod animate;
mod clipboard;
pub(crate) mod edit;
mod element;
mod feedback;
mod field;
mod host;
mod json;
mod layout;
mod overlay;
mod png;
mod virtualize;
pub use virtualize::virtual_list;
mod runtime;
#[cfg(feature = "inspect")]
pub use runtime::inspect;
mod rotate;
mod scroll;
pub mod token;

pub use element::{
    Align, ArcStart, Axis, El, Handled, Justify, Key, KeyCode, Len, Modifiers, Motion, Orientation, Paint,
    Face, Painter, PointerEvent, native, PointerPhase, TextAlign, Transition, Tx, WheelEvent, arc, col, el, grid, icon, image, paint, row, text, text_area,
    text_input,
};
pub use crate::platform::current::host::reduced_motion;
pub use clipboard::{Clipboard, Memory as MemoryClipboard};
/// The system clipboard: copy and paste with other apps outside fields (`set_clipboard`, or
/// an app's own "Copy" action).
pub use crate::platform::current::clipboard::System as SystemClipboard;
pub use host::{App, Input, Script, run};
/// What a trigger opens, as [`crate::Detail::popup`] tells assistive technology.
pub use accesskit::HasPopup;
/// Shadows for [`El::shadow_level`] and [`El::box_shadow`].
pub use crate::theme::{ShadowLevel, SoftShadow};
pub use overlay::{Overlay, Side, TOOLTIP_DELAY, dialog, menu, popover, toasts};
pub use runtime::{Change, Popup, Runtime, Shown, Snapshot};
