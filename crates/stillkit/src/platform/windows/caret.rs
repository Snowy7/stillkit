//! Caret stops for stillkit-drawn text fields (`el::text_input` when no RichEdit is attached):
//! where the caret stands before each character of one line, from DirectWrite's hit testing on
//! the line's cached layout (one layout, no re-shaping per character).

use windows::Win32::Graphics::DirectWrite::DWRITE_HIT_TEST_METRICS;

use super::Text;
use crate::TextStyle;

/// The caret's x (device px from the line's start) at every char boundary of `line`: one
/// entry per char, plus the end.
pub(crate) fn stops(text: &mut Text, line: &str, style: &TextStyle) -> Vec<f32> {
    let width = text.measure(line, style).0;
    let mut out = Vec::with_capacity(line.chars().count() + 1);
    let Some(layout) = text.layout(line, style) else {
        out.resize(line.chars().count() + 1, 0.0);
        return out;
    };
    let mut at = 0u32;
    for c in line.chars() {
        let (mut x, mut y) = (0.0, 0.0);
        let mut hit = DWRITE_HIT_TEST_METRICS::default();
        // SAFETY: a documented query on a live layout; the out pointers are locals.
        let ok = unsafe { layout.HitTestTextPosition(at, false, &mut x, &mut y, &mut hit) }.is_ok();
        out.push(if ok { x } else { out.last().copied().unwrap_or(0.0) });
        at += c.len_utf16() as u32;
    }
    out.push(width);
    out
}
