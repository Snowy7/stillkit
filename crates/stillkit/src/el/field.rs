//! A text field as the runtime keeps it between frames ([`super::text_input`],
//! [`super::text_area`]): its [`Editor`], its laid-out lines, its scroll and its caret blink.
//! The view never runs for caret moves, selection, scrolling or the blink; only an edit the
//! app takes (`on_input`) does.
//!
//! Lines come from one shaped layout per hard line ([`crate::platform::current::caret`]):
//! the caret's place before every character, so hit testing and the caret need no shaping of
//! their own, and wrapping (`text_area`) breaks at word boundaries on those places.

use std::sync::Arc;
use std::time::Duration;

use unicode_segmentation::UnicodeSegmentation;

use super::edit::{Editor, masked};
use crate::{Frame, Op, Point, Rect, Rgba, Text, TextStyle};

/// The caret blinks this long on and off (Windows' default caret blink time)...
pub(crate) const BLINK: Duration = Duration::from_millis(530);
/// ...and stops, shown, after this long without input, as Windows' caret does.
pub(crate) const BLINK_FOR: Duration = Duration::from_secs(5);

/// One visual line: where it starts and ends in the shown text, and the caret's place (x from
/// the line's start, device px) at each of its stops.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    pub start: usize,
    pub end: usize,
    /// Shown-text offset and x at each stop, the line's end included.
    pub stops: Vec<(usize, f32)>,
    /// What the line draws (bullets for a secure field).
    pub drawn: Arc<str>,
}

impl Line {
    fn width(&self) -> f32 {
        self.stops.last().map_or(0.0, |s| s.1)
    }

    /// The caret's x at `at` (the last stop at or before it).
    pub fn x(&self, at: usize) -> f32 {
        self.stops.iter().take_while(|s| s.0 <= at).last().map_or(0.0, |s| s.1)
    }

    /// The stop nearest `x`.
    pub fn at(&self, x: f32) -> usize {
        self.stops
            .iter()
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map_or(self.start, |s| s.0)
    }
}

/// A field's text laid out in its width.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Shape {
    pub lines: Vec<Line>,
    /// One line's height, device px.
    pub line: f32,
}

impl Shape {
    pub fn height(&self) -> f32 {
        self.line * self.lines.len().max(1) as f32
    }

    /// The visual line holding `at` (a wrapped line's end belongs to the next line).
    pub fn line_of(&self, at: usize) -> usize {
        self.lines.iter().rposition(|l| l.start <= at).unwrap_or(0)
    }
}

/// What a field looks like, from its element, resolved in device px.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Look {
    pub style: TextStyle,
    pub line: f32,
    pub ink: Rgba,
    pub multiline: bool,
    pub secure: bool,
    /// The element's laid-out rect, and its content box (less its padding).
    pub rect: Rect,
    pub content: Rect,
}

/// A field's retained state.
#[derive(Debug, Default)]
pub(crate) struct Field {
    pub ed: Editor,
    /// The value the view last gave, and the editor's state then: a value the view refuses
    /// to take goes back to it.
    pub value: Option<Arc<str>>,
    pub taken: Option<super::edit::Mark>,
    pub look: Option<Look>,
    pub shape: Shape,
    /// What `shape` was laid out from.
    shaped: Option<(String, Look)>,
    /// A one-line field's horizontal scroll, device px.
    pub scroll_x: f32,
    /// The x a run of Up and Down keeps to.
    pub goal: Option<f32>,
    /// When the caret last moved (its blink restarts): the runtime clock.
    pub blink: Duration,
    /// The last press: when, where, and how many in a row (double and triple clicks).
    pub clicks: Option<(Duration, Point, u8)>,
    /// The RichEdit typing into it while it has focus in a window (Windows).
    #[cfg(windows)]
    pub native: Option<crate::TextField>,
}

impl Field {
    pub fn new(text: &str) -> Self {
        Self {
            ed: Editor::new(text),
            ..Self::default()
        }
    }

    /// Lays the text out again if it or the look changed; whether it did.
    pub fn reshape(&mut self, text: &mut Text) -> bool {
        let Some(look) = &self.look else { return false };
        let shown = self.ed.shown().into_owned();
        if self.shaped.as_ref().is_some_and(|(t, l)| *t == shown && l == look) {
            return false;
        }
        let width = look.multiline.then_some(look.content.width());
        self.shape = shape(text, &shown, look, width);
        self.shaped = Some((shown, look.clone()));
        self.follow();
        true
    }

    /// Keeps a one-line field's caret in sight.
    pub fn follow(&mut self) {
        let Some(look) = &self.look else { return };
        let Some(line) = self.shape.lines.first().filter(|_| !look.multiline) else { return };
        let (x, room) = (line.x(self.ed.shown_caret()), look.content.width() - 1.0);
        if x - self.scroll_x > room {
            self.scroll_x = x - room;
        }
        if x < self.scroll_x {
            self.scroll_x = x;
        }
        self.scroll_x = self.scroll_x.min((line.width() - room).max(0.0)).max(0.0).round();
    }

    /// The caret's rect in the content box (device px, before any vertical scroll).
    pub fn caret(&self, s: f32) -> Rect {
        let at = self.ed.shown_caret();
        let n = self.shape.line_of(at);
        let x = self.shape.lines.get(n).map_or(0.0, |l| l.x(at));
        let look = self.look.as_ref();
        let top = self.top(look) + n as f32 * self.shape.line;
        let left = look.map_or(0.0, |l| l.content.left) + x - self.scroll_x;
        Rect::new(left.round(), top, s.round().max(1.0), self.shape.line)
    }

    /// Where the first line's top is (a one-line field centres its line).
    fn top(&self, look: Option<&Look>) -> f32 {
        look.map_or(0.0, |l| {
            if l.multiline {
                l.content.top
            } else {
                (l.content.top + (l.content.height() - self.shape.line) / 2.0).round()
            }
        })
    }

    /// The model offset under `p` (device px, `scroll_y` the text area's offset).
    pub fn hit(&self, p: Point, scroll_y: f32) -> usize {
        let look = self.look.as_ref();
        let y = p.y + scroll_y - self.top(look);
        let n = ((y / self.shape.line.max(1.0)).floor().max(0.0) as usize).min(self.shape.lines.len().saturating_sub(1));
        let Some(line) = self.shape.lines.get(n) else { return 0 };
        let x = p.x - look.map_or(0.0, |l| l.content.left) + self.scroll_x;
        self.model(line.at(x))
    }

    /// A shown-text offset back in the editor's text (a composition counts as the caret).
    fn model(&self, at: usize) -> usize {
        match &self.ed.preedit {
            Some((pre, _)) if at > self.ed.caret + pre.len() => at - pre.len(),
            Some(_) if at > self.ed.caret => self.ed.caret,
            _ => at,
        }
    }

    /// The field's lines, characters and selection for assistive technology (`scroll_y` a text
    /// area's offset). `None` while an IME composes (its text isn't the field's yet).
    pub fn accessible(&self, scroll_y: f32) -> Option<crate::FieldText> {
        let look = self.look.as_ref().filter(|_| self.ed.preedit.is_none())?;
        let top = self.top(Some(look)) - scroll_y;
        let left = look.content.left - self.scroll_x;
        let lines = self
            .shape
            .lines
            .iter()
            .enumerate()
            .map(|(n, l)| crate::TextLine {
                text: self.ed.text.get(l.start..l.end).unwrap_or("").into(),
                rect: Rect::new(left, top + n as f32 * self.shape.line, l.width(), self.shape.line),
                chars: l.stops.windows(2).map(|w| (w[0].1, w[1].1 - w[0].1)).collect(),
            })
            .collect();
        let place = |at: usize| {
            let n = self.shape.line_of(at);
            let k = self.shape.lines.get(n).map_or(0, |l| l.stops.iter().filter(|s| s.0 < at).count());
            (n, k)
        };
        Some(crate::FieldText {
            lines,
            anchor: place(self.ed.anchor),
            caret: place(self.ed.caret),
        })
    }

    /// The caret moved or the text changed: the blink restarts.
    pub fn touch(&mut self, now: Duration) {
        self.blink = now;
    }

    /// Whether the caret shows at `now` while focused (it blinks, then holds still).
    pub fn caret_on(&self, now: Duration) -> bool {
        let since = now.saturating_sub(self.blink);
        since >= BLINK_FOR || (since.as_millis() / BLINK.as_millis()) % 2 == 0
    }

    /// When the blink next changes, while focused; `None` once it holds still.
    pub fn next_blink(&self, now: Duration) -> Option<Duration> {
        let since = now.saturating_sub(self.blink);
        if since >= BLINK_FOR {
            return None;
        }
        let n = since.as_millis() / BLINK.as_millis() + 1;
        Some((self.blink + BLINK * n as u32).min(self.blink + BLINK_FOR))
    }
}

/// Lays `shown` out: one line, or wrapped at `width` (hard line breaks kept).
pub(crate) fn shape(text: &mut Text, shown: &str, look: &Look, width: Option<f32>) -> Shape {
    let mut lines = Vec::new();
    let mut start = 0;
    for hard in shown.split('\n') {
        lay_line(text, shown, start, hard, look, width, &mut lines);
        start += hard.len() + 1;
    }
    Shape { lines, line: look.line }
}

/// Lays one hard line (`hard`, at `start` in `shown`) out into visual lines.
fn lay_line(text: &mut Text, shown: &str, start: usize, hard: &str, look: &Look, width: Option<f32>, out: &mut Vec<Line>) {
    // Stops in shown offsets and in the drawn string, with x along the unwrapped line.
    let (drawn, offsets): (String, Vec<usize>) = if look.secure {
        let (d, o) = masked(hard);
        (d, o.into_iter().map(|o| start + o).collect())
    } else {
        let mut o: Vec<usize> = hard.char_indices().map(|(i, _)| start + i).collect();
        o.push(start + hard.len());
        (hard.to_owned(), o)
    };
    let xs = crate::platform::current::caret::stops(text, &drawn, &look.style);
    let bytes: Vec<usize> = drawn.char_indices().map(|(i, _)| i).chain([drawn.len()]).collect();
    let n = offsets.len().min(xs.len()).min(bytes.len());
    let line = |from: usize, to: usize| {
        let x0 = xs[from];
        Line {
            start: offsets[from],
            end: offsets[to],
            stops: (from..=to).map(|k| (offsets[k], xs[k] - x0)).collect(),
            drawn: drawn[bytes[from]..bytes[to]].into(),
        }
    };
    let Some(width) = width.filter(|_| n > 1) else {
        out.push(line(0, n - 1));
        return;
    };
    // Breaks may come before a word (not before the spaces after one, which hang).
    let words: Vec<usize> = hard
        .split_word_bound_indices()
        .filter(|(_, w)| !w.chars().all(char::is_whitespace))
        .map(|(i, _)| start + i)
        .collect();
    let space = |k: usize| shown[offsets[k - 1]..offsets[k]].chars().all(char::is_whitespace);
    let (mut from, mut brk) = (0, None);
    for k in 1..n {
        if xs[k] - xs[from] > width && k - 1 > from && !space(k) {
            let at = brk.filter(|&b| b > from).unwrap_or(k - 1);
            out.push(line(from, at));
            from = at;
            brk = None;
        }
        if words.contains(&offsets[k]) {
            brk = Some(k);
        }
    }
    out.push(line(from, n - 1));
}

/// Paints a field into `rect` (its laid-out box, where it is drawn now): selection, text or
/// placeholder, composition underline and caret, clipped to the box. `scroll_y` is a text
/// area's offset.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint(
    f: &mut Frame,
    field: &Field,
    rect: Rect,
    placeholder: Option<&str>,
    caret: Option<Rgba>,
    selection: Rgba,
    placeholder_ink: Rgba,
    scroll_y: f32,
) {
    let Some(look) = &field.look else { return };
    // Paint may run while the box springs: draw where the box is now.
    let (dx, dy) = (rect.left - look.rect.left, rect.top - look.rect.top);
    let origin = Point::new(look.content.left - field.scroll_x + dx, field.top(Some(look)) - scroll_y + dy);
    let line_h = field.shape.line;
    let mut ops = Vec::new();
    let ed = &field.ed;
    let sel = ed.selection();
    if caret.is_some() && !sel.is_empty() && ed.preedit.is_none() {
        for (n, l) in field.shape.lines.iter().enumerate() {
            let (a, b) = (sel.start.max(l.start), sel.end.min(l.end));
            let past = sel.end > l.end && sel.start <= l.end;
            if a > b || a == b && !past {
                continue;
            }
            let x0 = l.x(a);
            let x1 = l.x(b) + if past { (line_h / 4.0).round() } else { 0.0 };
            ops.push(Op::Fill {
                rect: Rect::new((origin.x + x0).round(), origin.y + n as f32 * line_h, (x1 - x0).round(), line_h),
                radius: 0.0,
                color: selection,
            });
        }
    }
    let empty = ed.text.is_empty() && ed.preedit.is_none();
    match placeholder.filter(|_| empty) {
        Some(p) => ops.push(Op::Text {
            rect: Rect::new(origin.x.round(), origin.y, look.content.width(), line_h),
            text: p.into(),
            style: look.style.clone(),
            color: placeholder_ink,
        }),
        None => {
            for (n, l) in field.shape.lines.iter().enumerate() {
                if l.drawn.is_empty() {
                    continue;
                }
                ops.push(Op::Text {
                    rect: Rect::new(origin.x.round(), origin.y + n as f32 * line_h, l.width().ceil(), line_h),
                    text: l.drawn.clone(),
                    style: look.style.clone(),
                    color: look.ink,
                });
            }
        }
    }
    if let Some((pre, _)) = &ed.preedit {
        let (a, b) = (ed.caret, ed.caret + pre.len());
        let n = field.shape.line_of(a);
        if let Some(l) = field.shape.lines.get(n) {
            let y = (origin.y + (n + 1) as f32 * line_h - f.s.max(1.0)).round();
            ops.push(Op::Line {
                from: Point::new(origin.x + l.x(a), y),
                to: Point::new(origin.x + l.x(b.min(l.end)), y),
                width: f.s.round().max(1.0),
                color: look.ink,
            });
        }
    }
    if let Some(color) = caret {
        let r = field.caret(f.s);
        ops.push(Op::Fill {
            rect: r.offset(dx, dy - scroll_y),
            radius: 0.0,
            color,
        });
    }
    // The caret may stand on the box's right edge.
    let clip = Rect::new(rect.left, rect.top, rect.width() + f.s.round().max(1.0), rect.height());
    f.push(Op::Clipped { rect: clip, ops: ops.into() });
}
