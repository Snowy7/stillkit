//! stillkit: a native UI library (docs/ui.md). Surfaces are drawn on the CPU by default
//! (software Direct2D and DirectWrite on a WIC bitmap, presented as layered windows), which loads
//! no GPU driver. GPU mode (`platform::Render::Gpu`, the `gpu` feature) draws the same picture
//! with hardware Direct2D and can draw Liquid Glass in-process; it is chosen per app or surface.
//!
//! A frame is `view(&model, &tokens) -> DisplayList`; [`Surface::present`] compares it with the
//! list on screen and redraws only the bounds that changed (nothing at all when it is the same).
//! Input arrives as [`Event`]s from [`next_event`], handled by the app's loop after dispatching
//! window messages. [`Menu`] is the first component built on it. Window class names come from
//! [`set_identity`].

#![warn(missing_docs)]

mod a11y;
pub mod platform;
mod identity;
pub use identity::{Identity, set_identity};
pub use a11y::{AccessibilityAction, Detail, FieldText, Semantics, TextLine};
pub use platform::current::app_window;
pub mod calendar;
pub mod chart;
pub mod color_row;
pub use platform::current::compose;
/// Legacy immediate-mode Studio controls; retained for mypaper. Prefer `el` and stillkit-kit.
#[doc(hidden)]
pub mod controls;
pub mod damage;
pub mod dialog;
pub mod el;
pub mod focus;
pub mod geom;
pub mod glass;
#[cfg(any(test, feature = "golden"))]
pub mod golden;
mod icon_paths;
pub mod icons;
pub mod interact;
pub mod interaction_motion;
pub mod kit;
pub use platform::current::layered_app;
pub mod layout;
pub mod menu;
pub use platform::current::modules;
pub mod motion;
pub use platform::current::render;
pub mod reveal;
mod scene;
mod shadow;
mod bitmap;
mod text_style;
pub mod scroll;
pub mod sortable;
use platform::current::surface;
use platform::current::text;
pub use platform::current::text_field;
pub mod theme;
mod tokens;
pub mod transition;
mod waker;
use platform::current::watch;

pub use app_window::{Chrome, Cursor, set_modal_hook};
pub use geom::{PixelRect, Point, Rect, Rgba};
pub use glass::{Glass, GlassMaterial};
pub use interact::Interaction;
pub use kit::{Accessory, ButtonKind, Frame, Lead, Row as ListRow, Tile};
pub use layout::{Align, Insets, SizeClass, Stack};
pub use menu::{Entry, Menu, MenuIcon, Outcome, Screen, screen_at};
pub use motion::{Spring, Ticker, Tween, display_frame};
pub use render::{Image, TextMode, TextQuality, Ui, render};
pub use scene::{DisplayList, Hit, HitId, Op};
pub use scroll::Scroll;
pub use surface::AnimationFrames;
pub use surface::{Button, Event, Kind, Pointer, Surface, SurfaceId, next_event, queue_event, take_dropped_files};
pub use platform::NativeHandle;
pub use platform::Caption;
pub use text::{Text, TextStyle, VARIABLE};
pub use text_field::TextField;
pub use theme::{Palette, Scheme, ShadowLevel, SoftShadow, Style, TextRole, Theme};
pub use tokens::{Colors, Ink, Material, MenuMetrics, Step, Tokens, Typeface, WINDOW_CORNER_RADIUS};
pub use watch::FileWatch;
pub use waker::{Waker, waker};

/// Accessibility roles for [`Hit`] items.
pub use accesskit::Role;
