//! Shared styling for overlay surfaces, resolved without inspecting look names.
use crate::{readable, surface};
use stillkit::el::{El, Paint, Transition, col, token};
pub(crate) use stillkit::el::ShadowLevel;
pub(crate) const INK: Paint =
    Paint::Theme(|t| readable(t.color.label, surface(t, t.color.elevated), 7.));
pub(crate) const DIM: Paint =
    Paint::Theme(|t| readable(t.color.secondary, surface(t, t.color.elevated), 4.5));
/// An overlay's body, casting the theme's shadow of `level`.
pub(crate) fn panel<S>(level: ShadowLevel) -> El<S> {
    col()
        .fill(token::ELEVATED)
        .radius(token::RADIUS_CARD)
        .shadow_level(level)
        .motion(token::MOTION_OPEN)
        .transition(Transition::scale(0.96))
}
