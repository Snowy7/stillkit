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
        out.push(if ok {
            x
        } else {
            out.last().copied().unwrap_or(0.0)
        });
        at += c.len_utf16() as u32;
    }
    out.push(width);
    out
}

/// The metrics and stops of the same slanted face used for drawing.
pub(crate) fn attributed(
    text: &mut Text,
    line: &str,
    style: &TextStyle,
    italic: bool,
) -> (f32, f32, f32, Vec<f32>) {
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_LINE_METRICS,
        DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    };
    let Some(layout) = text.layout(line, style) else {
        return (0.0, 0.0, 0.0, vec![0.0]);
    };
    let range = DWRITE_TEXT_RANGE {
        startPosition: 0,
        length: line.encode_utf16().count() as u32,
    };
    let mut metrics = DWRITE_TEXT_METRICS::default();
    let mut out = Vec::new();
    let mut lines = [DWRITE_LINE_METRICS::default()];
    let mut count = 0;
    // SAFETY: the cached layout is live; all output pointers refer to locals. Restore its shared face before returning.
    unsafe {
        let _ = layout.SetFontStyle(
            if italic {
                DWRITE_FONT_STYLE_ITALIC
            } else {
                DWRITE_FONT_STYLE_NORMAL
            },
            range,
        );
        let _ = layout.GetMetrics(&mut metrics);
        let _ = layout.GetLineMetrics(Some(&mut lines), &mut count);
        let mut at = 0;
        for c in line.chars() {
            let (mut x, mut y) = (0.0, 0.0);
            let mut hit = DWRITE_HIT_TEST_METRICS::default();
            let _ = layout.HitTestTextPosition(at, false, &mut x, &mut y, &mut hit);
            out.push(x);
            at += c.len_utf16() as u32;
        }
        let _ = layout.SetFontStyle(DWRITE_FONT_STYLE_NORMAL, range);
    }
    out.push(metrics.widthIncludingTrailingWhitespace);
    (
        metrics.widthIncludingTrailingWhitespace,
        metrics.height,
        lines[0].baseline,
        out,
    )
}
