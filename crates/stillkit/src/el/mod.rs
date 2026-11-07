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

mod animate;
mod element;
mod host;
mod json;
mod layout;
mod overlay;
mod png;
mod runtime;
mod scroll;
pub mod token;

pub use element::{
    Align, ArcStart, Axis, El, Handled, Justify, Key, KeyCode, Len, Motion, Orientation, Paint, Painter,
    Transition, Tx, arc, col, el, grid, icon, image, paint, row, text,
};
pub use crate::platform::current::host::reduced_motion;
pub use host::{App, Input, Script, run};
/// What a trigger opens, as [`crate::Detail::popup`] tells assistive technology.
pub use accesskit::HasPopup;
pub use overlay::{Overlay, Side, TOOLTIP_DELAY, dialog, menu, popover, toasts};
pub use runtime::{Change, Popup, Runtime, Shown, Snapshot};
