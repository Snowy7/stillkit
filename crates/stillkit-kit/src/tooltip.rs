//! Tooltip lifecycle and styling come from el: elevated fill, Footnote type, small radius,
//! shadow and presence fade on the theme's open spring. No duplicate hover timer is needed.
use stillkit::el::El;
/// Attach the runtime's themed, non-interactive tooltip to a trigger.
pub struct Tooltip;
impl Tooltip {
    pub fn attach<S>(trigger: El<S>, text: impl Into<std::sync::Arc<str>>) -> El<S> {
        trigger.tooltip(text)
    }
}
