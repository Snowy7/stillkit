//! Shared styling for overlay surfaces, resolved without inspecting look names.
use crate::{readable, surface};
use stillkit::el::{El, Paint, Transition, col, token};
pub(crate) const INK: Paint =
    Paint::Theme(|t| readable(t.color.label, surface(t, t.color.elevated), 7.));
pub(crate) const DIM: Paint =
    Paint::Theme(|t| readable(t.color.secondary, surface(t, t.color.elevated), 4.5));
pub(crate) fn panel<S>() -> El<S> {
    col()
        .fill(token::ELEVATED)
        .radius(token::RADIUS_CARD)
        .shadow()
        .motion(token::MOTION_OPEN)
        .transition(Transition::scale(0.96))
}
