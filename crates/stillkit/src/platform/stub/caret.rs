//! Compile-only caret stops: each prefix measured (see the Windows backend's `caret`).

use super::Text;
use crate::TextStyle;

pub(crate) fn stops(text: &mut Text, line: &str, style: &TextStyle) -> Vec<f32> {
    let mut out: Vec<f32> = line.char_indices().map(|(i, _)| text.measure(&line[..i], style).0).collect();
    out.push(text.measure(line, style).0);
    out
}

pub(crate) fn attributed(text: &mut Text, line: &str, style: &TextStyle, _italic: bool) -> (f32, f32, Vec<f32>) {
    let (w, h) = text.measure(line, style); (w, h, stops(text, line, style))
}
