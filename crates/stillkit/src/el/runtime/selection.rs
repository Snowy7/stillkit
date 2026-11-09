//! Selection of display text, retained independently of application state.
use super::super::{
    edit::{Cmd, Move},
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
    id: u64,
    rich: Option<super::super::rich::Layout>,
    selectable: bool,
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
            el.selectable || matches!(&el.what, What::Text { runs, .. } if !runs.is_empty()) || el.children.iter().any(enabled)
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
            .filter(|(_, e)| (e.selectable || matches!(&e.what, What::Text { runs, .. } if !runs.is_empty())) && !e.sem.disabled)
        {
            let What::Text { text, face, wrap, runs } = &el.what else {
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
                id: l.id,
                rich: None,
                selectable: el.selectable,
                doc,
                start,
                align: face.align,
            });
            d.doc = doc;
            d.selectable = el.selectable;
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
            d.rich = (!runs.is_empty()).then(|| super::super::rich::layout(&mut self.ui.text, runs, &style, line, self.scale, l.rect.width()));
            if let Some(rich) = &d.rich { d.field.shape = rich.shape.clone(); d.field.look.as_mut().expect("rich look").line = rich.shape.line; }
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
            .and_then(|h| self.index_of_hit(h).map(|i| &self.laid[i]))
            .and_then(|l| self.display.get(&l.id))
            .filter(|d| d.selectable)
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
        if let Some(cmd @ Cmd::Move(m, extend)) = super::super::edit::command(key, self.mods, false)
        {
            if matches!(m, Move::Home | Move::End) {
                let at = ed.caret;
                if let Some(d) = self
                    .display
                    .values()
                    .filter(|d| d.doc == id && d.start <= at)
                    .max_by_key(|d| d.start)
                {
                    let line = d.field.shape.line_of(at.saturating_sub(d.start));
                    if let Some(line) = d.field.shape.lines.get(line) {
                        ed.place(
                            d.start
                                + if m == Move::Home {
                                    line.start
                                } else {
                                    line.end
                                },
                            extend,
                        );
                    }
                }
            } else {
                ed.apply(&cmd, false);
            }
            self.sync_display_ranges();
            return true;
        }
        false
    }

    fn display_at(
        &self,
        p: Point,
        doc: Option<u64>,
        hit: Option<crate::HitId>,
    ) -> Option<(u64, usize)> {
        self.laid
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let d = self.display.get(&l.id)?;
                if hit.is_some_and(|h| self.index_of_hit(h) != Some(i)) {
                    return None;
                }
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

    pub(super) fn display_link_hit(&self, hit: crate::HitId) -> Option<(crate::HitId, std::sync::Arc<str>)> {
        for l in &self.laid {
            let Some(d) = self.display.get(&l.id) else { continue };
            let Some(rich) = &d.rich else { continue };
            for (i, fragment) in rich.fragments.iter().enumerate() {
                if link_id(d.id, i) == hit && let Some(url) = &fragment.run.link {
                    return Some((l.hit, url.clone()));
                }
            }
        }
        None
    }
    pub(super) fn display_link(&self, p: Point) -> Option<(crate::HitId, std::sync::Arc<str>)> {
        for (i, l) in self.laid.iter().enumerate() {
            let Some(d) = self.display.get(&l.id) else { continue };
            let Some(rich) = &d.rich else { continue };
            let rect = self.poses.get(i)?.rect;
            if p.x < rect.left || p.x > rect.right || p.y < rect.top || p.y > rect.bottom { continue; }
            for fragment in &rich.fragments {
                let r = super::super::rich::bounds(rich, fragment, rect, d.align);
                if p.x >= r.left && p.x <= r.right && p.y >= r.top && p.y <= r.bottom
                    && let Some(url) = &fragment.run.link { return Some((l.hit, url.clone())); }
            }
        }
        None
    }
    pub(super) fn display_pointer(&mut self, event: &Event) {
        if self.display.is_empty() {
            return;
        }
        match *event {
            Event::Press(p, Button::Left) | Event::DoubleClick(p, Button::Left) => {
                if !self.ix.pressed.is_some_and(|hit| {
                    self.index_of_hit(hit).is_some_and(|i| self.display.get(&self.laid[i].id).is_some_and(|d| d.selectable))
                }) {
                    self.display_drag = None;
                    return;
                }
                let pressed = self
                    .ix
                    .pressed
                    .and_then(|hit| self.index_of_hit(hit).map(|i| &self.laid[i]))
                    .and_then(|l| self.display.get(&l.id))
                    .map(|d| d.doc);
                let Some((doc, at)) = self.display_at(p.at, pressed, self.ix.pressed) else {
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
                let Some((_, at)) = self.display_at(p.at, Some(drag.doc), None) else {
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
            | Event::Deactivated => { self.display_drag = None;  },
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
        let top = rect.top + n as f32 * d.field.shape.line;
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
        if d.rich.is_none() { ops.push(Op::Text {
            rect: Rect::new(left, top + lead, rect.width(), h),
            text: line.drawn.clone(),
            style: look.style.clone(),
            color: ink,
        }); }
    }
    if let Some(rich) = &d.rich {
        ops.extend(super::super::rich::paint(rich, rect, ink, token::ACCENT.resolve(f.theme), token::FILL.resolve(f.theme), d.align));
        for (i, fragment) in rich.fragments.iter().enumerate().filter(|(_, f)| f.run.link.is_some()) {
            let bounds = super::super::rich::bounds(rich, fragment, rect, d.align).intersect(rect);
            if bounds.width() <= 0.0 || bounds.height() <= 0.0 { continue; }
            f.list.hits.push(crate::Hit { id: link_id(d.id, i),
                rect: bounds, role: crate::Role::Link,
                label: fragment.run.text.clone(), enabled: true, actionable: true, checked: None });
        }
    }
    f.push(Op::Clipped {
        rect,
        ops: ops.into(),
    });
}

fn link_id(id: u64, index: usize) -> crate::HitId { (id as u32).wrapping_add(0x80000000).wrapping_add(index as u32) }
