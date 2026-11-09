//! Attributed display text. Runs retain plain text for document selection and copying.
use super::{
    El,
    element::What,
    field::{Line, Shape},
};
use crate::{Op, Point, Rect, Rgba, Text, TextStyle};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// One attributed run in a paragraph. Unset properties inherit the element's face and ink.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// Plain text, including any spaces between runs.
    pub text: Arc<str>,
    /// Font weight, 100 through 900.
    pub weight: Option<u16>,
    /// Italic face.
    pub italic: bool,
    /// Underline decoration.
    pub underline: bool,
    /// Strikethrough decoration.
    pub strike: bool,
    /// Explicit ink; otherwise inherits the element's colour.
    pub color: Option<Rgba>,
    /// Explicit font family.
    pub family: Option<Arc<str>>,
    /// Font size in design pixels.
    pub size: Option<f32>,
    /// Code face and a subtle background.
    pub code: bool,
    /// Link destination passed to `El::on_link` without opening it.
    pub link: Option<Arc<str>>,
}
impl Run {
    /// Starts an unstyled run.
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self {
            text: text.into(),
            weight: None,
            italic: false,
            underline: false,
            strike: false,
            color: None,
            family: None,
            size: None,
            code: false,
            link: None,
        }
    }
    /// Uses a heavier face.
    pub fn bold(mut self) -> Self {
        self.weight = Some(700);
        self
    }
    /// Uses an italic face.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    /// Draws an underline.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
    /// Draws a strikethrough.
    pub fn strike(mut self) -> Self {
        self.strike = true;
        self
    }
    /// Uses code typography and background.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }
    /// Identifies a link; activation is owned by the app.
    pub fn link(mut self, url: impl Into<Arc<str>>) -> Self {
        self.link = Some(url.into());
        self
    }

    /// Sets an explicit font weight.
    pub fn weight(mut self, weight: u16) -> Self {
        self.weight = Some(weight.clamp(100, 900));
        self
    }
    /// Sets an explicit foreground colour.
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }
    /// Sets an installed font family.
    pub fn family(mut self, family: impl Into<Arc<str>>) -> Self {
        self.family = Some(family.into());
        self
    }
    /// Sets the font size in design pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }
    /// Uses the platform's monospace face without a code background.
    pub fn mono(self) -> Self {
        self.family(if cfg!(windows) {
            "Consolas"
        } else if cfg!(target_os = "macos") {
            "Menlo"
        } else {
            "monospace"
        })
    }
    fn style(&self, base: &TextStyle, scale: f32) -> TextStyle {
        let mut style = base.clone();
        if let Some(weight) = self.weight {
            style.weight = weight.clamp(100, 900);
        }
        if let Some(size) = self.size.filter(|s| s.is_finite() && *s > 0.0) {
            style.size = size * scale;
        }
        if let Some(family) = &self.family {
            style.family = family.clone();
        } else if self.code {
            style.family = if cfg!(windows) {
                "Consolas"
            } else if cfg!(target_os = "macos") {
                "Menlo"
            } else {
                "monospace"
            }
            .into();
        }
        style
    }
}
/// A wrapping attributed paragraph. Add `.selectable()` to join document selection.
pub fn rich_text<S>(runs: impl IntoIterator<Item = Run>) -> El<S> {
    let runs: Vec<Run> = runs.into_iter().collect();
    let plain: String = runs.iter().map(|r| &*r.text).collect();
    let mut el = super::text(plain);
    if let What::Text {
        runs: out, wrap, ..
    } = &mut el.what
    {
        *out = runs;
        *wrap = Some(usize::MAX);
    }
    el
}
impl<S> El<S> {
    /// Receives a clicked link destination. The library performs no network or shell action.
    pub fn on_link(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
        self.on_link = Some(Arc::new(handler));
        self
    }
}
#[derive(Clone)]
pub(crate) struct Fragment {
    pub run: Run,
    pub line: usize,
    pub style: TextStyle,
    pub rect: Rect,
}
#[derive(Default)]
pub(crate) struct Layout {
    pub shape: Shape,
    pub fragments: Vec<Fragment>,
}
/// Shapes contiguous word pieces through the platform's native shaper.
pub(crate) fn layout(
    text: &mut Text,
    runs: &[Run],
    base: &TextStyle,
    line: f32,
    scale: f32,
    width: f32,
) -> Layout {
    let (mut ascent, mut descent) = (0f32, 0f32);
    for run in runs {
        let (_, height, baseline, _) = crate::platform::current::caret::attributed(
            text,
            "Ag",
            &run.style(base, scale),
            run.italic,
        );
        ascent = ascent.max(baseline);
        descent = descent.max(height - baseline);
    }
    let line = line.max(ascent + descent);
    let baseline = (line - ascent - descent) / 2.0 + ascent;
    let mut out = Layout {
        shape: Shape {
            lines: Vec::new(),
            line,
        },
        fragments: Vec::new(),
    };
    let (mut x, mut y, mut offset, mut start) = (0.0, 0.0, 0, 0);
    let mut stops = vec![(0, 0.0)];
    let mut drawn = String::new();
    for run in runs {
        let style = run.style(base, scale);
        for piece in run.text.split_word_bounds() {
            if piece.contains('\n') {
                for c in piece.chars() {
                    if c == '\n' {
                        out.shape.lines.push(Line {
                            start,
                            end: offset,
                            stops: std::mem::take(&mut stops),
                            drawn: std::mem::take(&mut drawn).into(),
                        });
                        offset += 1;
                        start = offset;
                        x = 0.0;
                        y += line;
                        stops.push((offset, 0.0));
                    } else {
                        offset += c.len_utf8();
                    }
                }
                continue;
            }
            let w = crate::platform::current::caret::attributed(text, piece, &style, run.italic).0;
            let pieces: Vec<&str> = if w > width.max(1.0) {
                piece.graphemes(true).collect()
            } else {
                vec![piece]
            };
            for part in pieces {
                let (w, h, run_baseline, xs) =
                    crate::platform::current::caret::attributed(text, part, &style, run.italic);
                if x > 0.0 && x + w > width && !part.chars().all(char::is_whitespace) {
                    out.shape.lines.push(Line {
                        start,
                        end: offset,
                        stops: std::mem::take(&mut stops),
                        drawn: std::mem::take(&mut drawn).into(),
                    });
                    start = offset;
                    x = 0.0;
                    y += line;
                    stops.push((offset, x));
                }
                for ((i, _), dx) in part.char_indices().zip(xs) {
                    stops.push((offset + i, x + dx));
                }
                let mut fragment = run.clone();
                fragment.text = part.into();
                out.fragments.push(Fragment {
                    run: fragment,
                    line: (y / line) as usize,
                    style: style.clone(),
                    rect: Rect::new(x, y + baseline - run_baseline, w, h),
                });
                offset += part.len();
                x += w;
                stops.push((offset, x));
                drawn.push_str(part);
            }
        }
    }
    out.shape.lines.push(Line {
        start,
        end: offset,
        stops,
        drawn: drawn.into(),
    });
    out
}
pub(crate) fn paint(
    layout: &Layout,
    rect: Rect,
    ink: Rgba,
    accent: Rgba,
    background: Rgba,
    align: super::element::TextAlign,
) -> Vec<Op> {
    let mut ops = Vec::new();
    for fragment in &layout.fragments {
        let run = &fragment.run;
        let rect = bounds(layout, fragment, rect, align);
        let color = run
            .color
            .unwrap_or(if run.link.is_some() { accent } else { ink });
        if run.code {
            ops.push(Op::Fill {
                rect,
                radius: 2.0,
                color: background,
            });
        }
        ops.push(Op::Emphasis {
            rect,
            text: run.text.clone(),
            style: fragment.style.clone(),
            color,
            italic: run.italic,
            strike: run.strike,
        });
        if run.underline || run.link.is_some() {
            ops.push(Op::Line {
                from: Point::new(rect.left, rect.bottom - 1.0),
                to: Point::new(rect.right, rect.bottom - 1.0),
                width: 1.0,
                color,
            });
        }
    }
    ops
}

pub(crate) fn bounds(
    layout: &Layout,
    fragment: &Fragment,
    rect: Rect,
    align: super::element::TextAlign,
) -> Rect {
    let width = layout
        .shape
        .lines
        .get(fragment.line)
        .map_or(0.0, |l| l.stops.iter().map(|s| s.1).fold(0.0, f32::max));
    let left = match align {
        super::element::TextAlign::Start => rect.left,
        super::element::TextAlign::Center => rect.left + (rect.width() - width) / 2.0,
        super::element::TextAlign::End => rect.right - width,
    };
    fragment.rect.offset(left, rect.top)
}
