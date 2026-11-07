//! macOS draws the bundled Lucide outlines, as Linux does: no Windows icon font here.
use crate::{Op, Rect, Rgba, Text, icons::Icon};
use std::sync::Arc;

/// The system text family (SF Pro, which CoreText resolves to the UI font).
pub fn system_family(_: &Text) -> Arc<str> {
    super::text::SYSTEM.into()
}

/// `icon` at `size` device pixels, centred in `rect`.
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
