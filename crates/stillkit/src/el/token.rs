//! Theme tokens for element builders: `col().pad(token::PAD).fill(token::CARD)`. Each reads
//! the theme the runtime holds, so a new look restyles every view without code changes. Make
//! your own with `Len::Theme(|t| ..)` and `Paint::Theme(|t| ..)`.

use super::{Len, Motion, Paint};

// ---- Lengths (design px) -------------------------------------------------------------------

/// A surface's padding.
pub const PAD: Len = Len::Theme(|t| t.metrics.pad);
/// The space between a surface's blocks.
pub const GAP: Len = Len::Theme(|t| t.metrics.gap);
/// The space between the items of a list or a row of controls (a gap, not a height).
pub const ITEM: Len = Len::Theme(|t| t.metrics.item);
/// A one-line list row's height.
pub const ROW_SINGLE: Len = Len::Theme(|t| t.metrics.row_single);
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
/// An arc's stroke ([`super::arc`]): twice the theme's icon stroke at 24 px, so rings get
/// heavier where icons do.
pub const RING: Len = Len::Theme(|t| t.icon_weight * 2.0);

// ---- Radii (design px) ---------------------------------------------------------------------

/// Radius surface.
pub const RADIUS_SURFACE: Len = Len::Theme(|t| t.radius.surface);
/// Radius card.
pub const RADIUS_CARD: Len = Len::Theme(|t| t.radius.card);
/// Radius small.
pub const RADIUS_SMALL: Len = Len::Theme(|t| t.radius.small);
/// The theme's control radius, or a capsule.
pub const RADIUS_CONTROL: Len = Len::Theme(|t| t.radius.control.unwrap_or(f32::INFINITY));

// ---- Colours -------------------------------------------------------------------------------

/// Label.
pub const LABEL: Paint = Paint::Theme(|t| t.color.label);
/// Secondary.
pub const SECONDARY: Paint = Paint::Theme(|t| t.color.secondary);
/// Tertiary.
pub const TERTIARY: Paint = Paint::Theme(|t| t.color.tertiary);
/// Surface.
pub const SURFACE: Paint = Paint::Theme(|t| t.color.surface);
/// Elevated.
pub const ELEVATED: Paint = Paint::Theme(|t| t.color.elevated);
/// Card.
pub const CARD: Paint = Paint::Theme(|t| t.color.card);
/// The neutral control fill.
pub const FILL: Paint = Paint::Theme(|t| t.color.fill);
/// Well.
pub const WELL: Paint = Paint::Theme(|t| t.color.well);
/// Separator.
pub const SEPARATOR: Paint = Paint::Theme(|t| t.color.separator);
/// Accent.
pub const ACCENT: Paint = Paint::Theme(|t| t.color.accent);
/// On accent.
pub const ON_ACCENT: Paint = Paint::Theme(|t| t.color.on_accent);
/// A switch's or slider's knob.
pub const KNOB: Paint = Paint::Theme(|t| t.color.knob);
/// A text field's caret: the accent.
pub const CARET: Paint = Paint::Theme(|t| t.color.accent);
/// A text field's selection: the accent, light enough to read the text through.
pub const SELECTION: Paint = Paint::Theme(|t| t.color.accent.with_alpha(if t.dark() { 0.45 } else { 0.3 }));

// ---- Motion (springs, [`El::motion`](super::El::motion)) -----------------------------------

/// Layout moves and morphs.
pub const MOTION_LAYOUT: Motion = Motion::Theme(|t| t.motion.layout);
/// A tight follow of a press or a toggle (a switch's knob).
pub const MOTION_INTERACTIVE: Motion = Motion::Theme(|t| t.motion.interactive);
/// Opening, closing, entering and exiting.
pub const MOTION_OPEN: Motion = Motion::Theme(|t| t.motion.open);

/// The dim behind a modal dialog ([`super::dialog`]): black, heavier on a dark scheme.
pub const SCRIM: Paint = Paint::Theme(|t| crate::Rgba::new(0.0, 0.0, 0.0, if t.dark() { 0.5 } else { 0.3 }));
