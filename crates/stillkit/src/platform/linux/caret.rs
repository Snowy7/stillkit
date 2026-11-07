//! Caret stops for stillkit-drawn text fields: where the caret stands before each character
//! of one line, from the line's cached cosmic-text shaping (glyph clusters carry their byte
//! ranges; characters inside a cluster share its advance evenly).

use super::Text;
use crate::TextStyle;

/// The caret's x (device px from the line's start) at every char boundary of `line`: one
/// entry per char, plus the end.
pub(crate) fn stops(text: &mut Text, line: &str, style: &TextStyle) -> Vec<f32> {
    let chars: Vec<usize> = line.char_indices().map(|(i, _)| i).collect();
    let mut out = vec![f32::NAN; chars.len() + 1];
    let mut end = 0f32;
    for run in text.shape(line, style).layout_runs() {
        end = end.max(run.line_w);
        for g in run.glyphs {
            let inside: Vec<usize> = (0..chars.len()).filter(|&k| chars[k] >= g.start && chars[k] < g.end).collect();
            let n = inside.len().max(1) as f32;
            for (j, &k) in inside.iter().enumerate() {
                let along = g.w * j as f32 / n;
                out[k] = if g.level.is_rtl() { g.x + g.w - along } else { g.x + along };
            }
        }
    }
    out[chars.len()] = end;
    let mut last = 0.0;
    for x in &mut out {
        if x.is_nan() {
            *x = last;
        }
        last = *x;
    }
    out
}
