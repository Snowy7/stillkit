//! Caret stops for stillkit-drawn text fields: where CoreText puts the caret before each
//! character of one line (its primary offset, from the line's cached shaping).
use super::Text;
use crate::TextStyle;

/// The caret's x (device px from the line's start) at every char boundary of `line`: one
/// entry per char, plus the end.
pub(crate) fn stops(text: &mut Text, line: &str, style: &TextStyle) -> Vec<f32> {
    let layout = text.shape(line, style, false);
    let Some((ct, width)) = layout.lines.first() else { return vec![0.0] };
    let mut out = Vec::with_capacity(line.len() + 1);
    let mut utf16 = 0;
    for c in line.chars() {
        // SAFETY: a live line and an index inside its string; no secondary offset is asked for.
        out.push(unsafe { ct.offset_for_string_index(utf16, std::ptr::null_mut()) } as f32);
        utf16 += c.len_utf16() as isize;
    }
    out.push(*width as f32);
    out
}

pub(crate) fn attributed(text: &mut Text, line: &str, style: &TextStyle, italic: bool) -> (f32, f32, Vec<f32>) {
    let layout = text.shape(line, style, italic);
    let mut out = Vec::new();
    if let Some((ct, width)) = layout.lines.first() {
        let mut at = 0;
        for c in line.chars() {
            // SAFETY: the line is live and the UTF-16 index is inside its string.
            out.push(unsafe { ct.offset_for_string_index(at, std::ptr::null_mut()) } as f32);
            at += c.len_utf16() as isize;
        }
        out.push(*width as f32);
    }
    (layout.width, layout.height(), out)
}
