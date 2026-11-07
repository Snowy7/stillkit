use crate::icons::{Icon, SYSTEM_FAMILY};
use crate::{Op, Rect, Rgba, Text, TextStyle};
use std::sync::Arc;
const SYSTEM_FALLBACK: &str = "Segoe MDL2 Assets";
/// The system icon font that is installed here.
pub fn system_family(text: &Text) -> Arc<str> {
    if text.has_family(SYSTEM_FAMILY) {
        SYSTEM_FAMILY.into()
    } else {
        SYSTEM_FALLBACK.into()
    }
}

/// `icon` at `size` device pixels, centred in `rect` by its measured box (a font glyph).
pub fn centered(
    text: &mut Text,
    family: &str,
    icon: Icon,
    size: f32,
    rect: Rect,
    color: Rgba,
) -> Op {
    let style = TextStyle::new(family, size);
    let glyph: Arc<str> = icon.glyph_in(family).to_string().into();
    let (w, h) = text.measure(&glyph, &style);
    Op::Text {
        rect: Rect::new(
            (rect.left + (rect.width() - w) / 2.0).round(),
            (rect.top + (rect.height() - h) / 2.0).round(),
            w,
            h,
        ),
        text: glyph,
        style,
        color,
    }
}
