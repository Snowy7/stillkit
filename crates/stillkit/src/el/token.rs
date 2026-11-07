//! Theme tokens for element builders: `col().pad(token::PAD).fill(token::CARD)`. Each reads
//! the theme the runtime holds, so a new look restyles every view without code changes. Make
//! your own with `Len::Theme(|t| ..)` and `Paint::Theme(|t| ..)`.

use super::{Len, Paint};

// ---- Lengths (design px) -------------------------------------------------------------------

/// A surface's padding.
pub const PAD: Len = Len::Theme(|t| t.metrics.pad);
/// The space between a surface's blocks.
pub const GAP: Len = Len::Theme(|t| t.metrics.gap);
/// A menu or list item's height.
pub const ITEM: Len = Len::Theme(|t| t.metrics.item);
/// A list row's height (two lines).
pub const ROW: Len = Len::Theme(|t| t.metrics.row);
/// A control's height (buttons, fields).
pub const CONTROL: Len = Len::Theme(|t| t.metrics.control);
/// A small control's height.
pub const CONTROL_SMALL: Len = Len::Theme(|t| t.metrics.control_small);
/// An icon's side.
pub const ICON: Len = Len::Theme(|t| t.metrics.icon);
/// The smallest pointer target.
pub const HIT: Len = Len::Theme(|t| t.metrics.hit);

// ---- Radii (design px) ---------------------------------------------------------------------

pub const RADIUS_SURFACE: Len = Len::Theme(|t| t.radius.surface);
pub const RADIUS_CARD: Len = Len::Theme(|t| t.radius.card);
pub const RADIUS_SMALL: Len = Len::Theme(|t| t.radius.small);
/// The theme's control radius, or a capsule.
pub const RADIUS_CONTROL: Len = Len::Theme(|t| t.radius.control.unwrap_or(f32::INFINITY));

// ---- Colours -------------------------------------------------------------------------------

pub const LABEL: Paint = Paint::Theme(|t| t.color.label);
pub const SECONDARY: Paint = Paint::Theme(|t| t.color.secondary);
pub const TERTIARY: Paint = Paint::Theme(|t| t.color.tertiary);
pub const SURFACE: Paint = Paint::Theme(|t| t.color.surface);
pub const ELEVATED: Paint = Paint::Theme(|t| t.color.elevated);
pub const CARD: Paint = Paint::Theme(|t| t.color.card);
/// The neutral control fill.
pub const FILL: Paint = Paint::Theme(|t| t.color.fill);
pub const WELL: Paint = Paint::Theme(|t| t.color.well);
pub const SEPARATOR: Paint = Paint::Theme(|t| t.color.separator);
pub const ACCENT: Paint = Paint::Theme(|t| t.color.accent);
pub const ON_ACCENT: Paint = Paint::Theme(|t| t.color.on_accent);
/// A switch's or slider's knob.
pub const KNOB: Paint = Paint::Theme(|t| t.color.knob);
