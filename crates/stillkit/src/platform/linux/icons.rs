//! Linux uses the bundled Lucide outlines instead of Windows private-use icon fonts.
use crate::{Op, Rect, Rgba, Text, icons::Icon};
use std::sync::Arc;
/// System family.
pub fn system_family(_: &Text) -> Arc<str> {
    "Inter".into()
}
/// Centered.
pub fn centered(_: &mut Text, _: &str, icon: Icon, size: f32, rect: Rect, color: Rgba) -> Op {
    Op::Icon {
        rect: Rect::new(
            (rect.left + (rect.width() - size) / 2.).round(),
            (rect.top + (rect.height() - size) / 2.).round(),
            size,
            size,
        ),
        icon,
        weight: (size / 12.).max(1.),
        color,
    }
}
