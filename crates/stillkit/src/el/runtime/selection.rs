//! Selection of display text, retained independently of application state.
use super::super::{
    edit::Cmd,
    element::{KeyCode, What},
    field::{Field, Look},
    token,
};
use super::{Runtime, flatten};
use crate::{Button, Event, Frame, Op, Point, Rect, Rgba};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::time::Duration;

pub(super) struct Display {
    pub field: Field,
    doc: u64,
    start: usize,
    align: super::super::element::TextAlign,
}

impl Display {
    pub(super) fn accessible(&self, rect: Rect) -> Option<crate::FieldText> {
        let mut text = self.field.accessible(0.0)?;
        let original = self.field.look.as_ref()?.rect;
        for line in &mut text.lines {
            line.rect = Rect::new(
                super::align_x(rect, line.rect.width(), self.align),
                line.rect.top + rect.top - original.top,
                line.rect.width(),
                line.rect.height(),
            );
        }
        Some(text)
    }
}

#[derive(Clone)]
pub(super) struct Drag {
    doc: u64,
    clicks: u8,
    from: Range<usize>,
}

impl<S> Runtime<S> {
    pub(super) fn sync_display(&mut self) {
        fn enabled<S>(el: &super::super::El<S>) -> bool {
            el.selectable || el.children.iter().any(enabled)
        }
        if self.display.is_empty() && !enabled(&self.root) {
            return;
        }
        let flat = flatten(&self.root);
        let mut texts: HashMap<u64, String> = HashMap::new();
        let mut seen = HashSet::new();
        for (i, el) in flat
            .iter()
            .enumerate()
            .filter(|(_, e)| e.selectable && !e.sem.disabled)
        {
            let What::Text { text, face, wrap } = &el.what else {
                continue;
            };
            // Controls keep their input even when added after `selectable()`.
            let mut ancestor = Some(i);
            let mut excluded = false;
            while let Some(at) = ancestor {
                let node = flat[at];
                if node.sem.disabled
                    || node.field().is_some()
                    || node.on_press.is_some()
                    || node.on_drag.is_some()
                    || node.on_pointer.is_some()
                {
                    excluded = true;
                    break;
                }
                ancestor = self.laid[at].parent;
            }
            if excluded {
                continue;
            }
            let l = self.laid[i];
            let mut root = i;
            while let Some(parent) = self.laid[root].parent.filter(|&p| flat[p].selectable) {
                root = parent;
            }
            let doc = self.laid[root].id;
            let first = !texts.contains_key(&doc);
            let content = texts.entry(doc).or_default();
            if !first {
                content.push('\n');
            }
            let start = content.len();
            content.push_str(text);
            seen.insert(l.id);
            let d = self.display.entry(l.id).or_insert_with(|| Display {
                field: Field::new(text),
                doc,
                start,
                align: face.align,
            });
            d.doc = doc;
            d.start = start;
            d.align = face.align;
            d.field.ed.set_text(text);
            let style = face.style(&self.theme, self.scale);
            let line = face.line(&self.theme, self.scale);
            d.field.look = Some(Look {
                style: style.clone(),
                line,
                ink: el.look.color.unwrap_or(token::LABEL).resolve(&self.theme),
                multiline: true,
                secure: false,
                rect: l.rect,
                content: l.rect,
            });
            d.field.shape = super::super::field::shape(
                &mut self.ui.text,
                text,
                d.field.look.as_ref().expect("display look"),
                Some(l.rect.width()),
            );
            d.field.shape.lines.truncate(wrap.unwrap_or(usize::MAX));
        }
        self.display.retain(|id, _| seen.contains(id));
        self.documents.retain(|id, _| texts.contains_key(id));
        for (id, text) in texts {
            let ed = self.documents.entry(id).or_default();
            if ed.text != text {
                ed.set_text(&text);
                self.display_drag = None;
            }
        }
        self.sync_display_ranges();
    }

    fn sync_display_ranges(&mut self) {
        for d in self.display.values_mut() {
            if let Some(ed) = self.documents.get(&d.doc) {
                let local = |n: usize| n.saturating_sub(d.start).min(d.field.ed.text.len());
                d.field.ed.select(local(ed.anchor)..local(ed.caret));
            }
        }
        self.stale_paint = true;
    }

    pub(super) fn display_key(&mut self, key: KeyCode) -> bool {
        if self.display.is_empty() {
            return false;
        }
        let Some(id) = self
            .ix
            .focus
            .and_then(|h| self.laid.iter().find(|l| l.hit == h))
            .and_then(|l| self.display.get(&l.id))
            .map(|d| d.doc)
        else {
            return false;
        };
        let Some(ed) = self.documents.get_mut(&id) else {
            return false;
        };
        if (self.mods.ctrl || self.mods.meta) && !self.mods.alt {
            if key == KeyCode::of('C') {
                if !ed.selected().is_empty() {
                    self.clipboard.set(ed.selected());
                }
                return true;
            }
            if key == KeyCode::of('A') {
                ed.select(0..ed.text.len());
                self.sync_display_ranges();
                return true;
            }
        }
        if let Some(cmd @ Cmd::Move(..)) = super::super::edit::command(key, self.mods, false) {
            ed.apply(&cmd, false);
            self.sync_display_ranges();
            return true;
        }
        false
    }

    fn display_at(&self, p: Point, doc: Option<u64>) -> Option<(u64, usize)> {
        self.laid
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let d = self.display.get(&l.id)?;
                if doc.is_some_and(|id| id != d.doc) {
                    return None;
                }
                let rect = self.poses.get(i)?.rect;
                if doc.is_none()
                    && !(p.x >= rect.left
                        && p.x <= rect.right
                        && p.y >= rect.top
                        && p.y <= rect.bottom)
                {
                    return None;
                }
                let dx = (rect.left - p.x).max(0.0) + (p.x - rect.right).max(0.0);
                let dy = (rect.top - p.y).max(0.0) + (p.y - rect.bottom).max(0.0);
                let distance = dx * dx + dy * dy;
                let line = ((p.y - rect.top) / d.field.shape.line.max(1.0))
                    .floor()
                    .max(0.0) as usize;
                let line = d
                    .field
                    .shape
                    .lines
                    .get(line.min(d.field.shape.lines.len().saturating_sub(1)))?;
                let width = line.stops.iter().map(|s| s.1).fold(0.0, f32::max);
                let left = super::align_x(rect, width, d.align);
                let at = line.at(p.x - left);
                Some((distance, d.doc, d.start + at))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, doc, at)| (doc, at))
    }

    pub(super) fn display_pointer(&mut self, event: &Event) {
        if self.display.is_empty() {
            return;
        }
        match *event {
            Event::Press(p, Button::Left) | Event::DoubleClick(p, Button::Left) => {
                if !self.ix.pressed.is_some_and(|hit| {
                    self.laid
                        .iter()
                        .any(|l| l.hit == hit && self.display.contains_key(&l.id))
                }) {
                    self.display_drag = None;
                    return;
                }
                let pressed = self
                    .ix
                    .pressed
                    .and_then(|hit| self.laid.iter().find(|l| l.hit == hit))
                    .and_then(|l| self.display.get(&l.id))
                    .map(|d| d.doc);
                let Some((doc, at)) = self.display_at(p.at, pressed) else {
                    self.display_drag = None;
                    return;
                };
                let clicks = if matches!(event, Event::DoubleClick(..)) {
                    2
                } else {
                    match self.display_clicks {
                        Some((time, last, n))
                            if self.clock.saturating_sub(time) <= Duration::from_millis(500)
                                && (last.x - p.at.x).abs() <= 4.0 * self.scale
                                && (last.y - p.at.y).abs() <= 4.0 * self.scale =>
                        {
                            n % 3 + 1
                        }
                        _ => 1,
                    }
                };
                self.display_clicks = Some((self.clock, p.at, clicks));
                let ed = self.documents.get_mut(&doc).expect("display document");
                let from = match clicks {
                    2 => ed.word_at(at),
                    3 => ed.line_at(at),
                    _ => at..at,
                };
                if self.held.shift {
                    ed.place(at, true);
                } else {
                    ed.select(from.clone());
                }
                let from = if self.held.shift {
                    ed.anchor..ed.anchor
                } else {
                    from
                };
                self.display_drag = Some(Drag { doc, clicks, from });
                self.sync_display_ranges();
            }
            Event::Move(p) => {
                let Some(drag) = self.display_drag.clone() else {
                    return;
                };
                let Some((_, at)) = self.display_at(p.at, Some(drag.doc)) else {
                    return;
                };
                let Some(ed) = self.documents.get_mut(&drag.doc) else {
                    return;
                };
                let unit = match drag.clicks {
                    2 => ed.word_at(at),
                    3 => ed.line_at(at),
                    _ => at..at,
                };
                if at < drag.from.start {
                    ed.select(drag.from.end..unit.start);
                } else {
                    ed.select(drag.from.start..unit.end.max(drag.from.end));
                }
                self.sync_display_ranges();
            }
            Event::Release(..)
            | Event::Cancelled
            | Event::CaptureLost
            | Event::Leave
            | Event::Deactivated => self.display_drag = None,
            _ => {}
        }
    }
}

pub(super) fn paint(f: &mut Frame, d: &Display, rect: Rect, ink: Rgba) {
    let Some(look) = &d.field.look else { return };
    let range = d.field.ed.selection();
    let mut ops = Vec::new();
    let (_, h) = f.text.measure("Ag", &look.style);
    let lead = ((look.line - h) / 2.0).round();
    for (n, line) in d.field.shape.lines.iter().enumerate() {
        let top = rect.top + n as f32 * look.line;
        let width = line.stops.iter().map(|s| s.1).fold(0.0, f32::max);
        let left = super::align_x(rect, width, d.align);
        let a = range.start.max(line.start);
        let b = range.end.min(line.end);
        if a < b {
            let x = line.x(a);
            ops.push(Op::Fill {
                rect: Rect::new(
                    left + x.min(line.x(b)),
                    top,
                    (line.x(b) - x).abs(),
                    look.line,
                ),
                radius: 0.0,
                color: token::SELECTION.resolve(f.theme),
            });
        }
        ops.push(Op::Text {
            rect: Rect::new(left, top + lead, rect.width(), h),
            text: line.drawn.clone(),
            style: look.style.clone(),
            color: ink,
        });
    }
    f.push(Op::Clipped {
        rect,
        ops: ops.into(),
    });
}
