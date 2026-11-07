//! Layout: each element is a retained Taffy node, found again by its identity (its key, or its
//! place among unkeyed siblings, under its parent's identity). A view run restyles only the
//! nodes whose style, text or children changed, so Taffy re-lays out only what they touch.
//! Lengths are device pixels, each rounded like [`crate::Frame::px`]; Taffy rounds the result
//! to whole pixels.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use taffy::Overflow;
use taffy::prelude::{
    AlignItems, AvailableSpace, Dimension, Display, FlexDirection, JustifyContent, LengthPercentage,
    LengthPercentageAuto, NodeId, Position, Style, TaffyTree, fr, repeat,
};

use super::element::{Align, Axis, Dim, El, Flow, Justify, Len, What};
use super::token;
use crate::{HitId, Rect, Text, TextStyle, Theme};

/// What a leaf measures through: text, through the shared [`Text`] cache.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Measure {
    text: Arc<str>,
    style: TextStyle,
    line: f32,
    wrap: Option<usize>,
}

struct Retained {
    node: NodeId,
    style: Style,
    measure: Option<Measure>,
    /// A scroll view's content node (its children hang from it) and its style.
    content: Option<(NodeId, Style)>,
    children: Vec<NodeId>,
    seen: u64,
}

/// One element after layout, in paint (pre-)order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Laid {
    /// The element's identity (stable across frames).
    pub id: u64,
    pub hit: HitId,
    /// Device pixels, in the surface.
    pub rect: Rect,
    /// The parent's place in the same list (`None` for the root).
    pub parent: Option<usize>,
    /// A scroll view's content size (device px): how far it scrolls is this less `rect`'s.
    pub extent: Option<(f32, f32)>,
}

/// The retained Taffy tree of one runtime.
pub(crate) struct Layout {
    taffy: TaffyTree<Measure>,
    nodes: HashMap<u64, Retained>,
    generation: u64,
    /// Nodes created, restyled, re-measured or re-parented by the last [`Layout::sync`].
    pub touched: usize,
}

/// What lengths resolve against.
pub(crate) struct Cx<'a> {
    pub theme: &'a Theme,
    pub s: f32,
}

impl Cx<'_> {
    fn px(&self, len: Len) -> f32 {
        (len.resolve(self.theme) * self.s).round()
    }
}

/// The identity of a child: its key, or its place, under its parent's identity.
pub(crate) fn child_id(parent: u64, key: Option<&str>, index: usize) -> u64 {
    let mut h = DefaultHasher::new();
    parent.hash(&mut h);
    match key {
        Some(key) => (0u8, key).hash(&mut h),
        None => (1u8, index).hash(&mut h),
    }
    h.finish()
}

/// The root's identity.
pub(crate) const ROOT: u64 = 0x5717_1c17;

/// A hit id from an identity: stable across frames, never [`crate::kit::NONE`].
pub(crate) fn hit_of(id: u64) -> HitId {
    match (id ^ (id >> 32)) as u32 {
        0 => 1,
        hit => hit,
    }
}

impl Layout {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            nodes: HashMap::new(),
            generation: 0,
            touched: 0,
        }
    }

    /// Brings the retained nodes in line with `root` (sized `size` device px) and lays them
    /// out. Returns every element in pre-order with its rect.
    pub fn update<S>(
        &mut self,
        root: &El<S>,
        size: (f32, f32),
        cx: &Cx,
        text: &mut Text,
    ) -> Vec<Laid> {
        self.generation += 1;
        self.touched = 0;
        let mut order = Vec::new();
        let node = self.sync(root, ROOT, cx, Some(size), &mut order);
        let generation = self.generation;
        let taffy = &mut self.taffy;
        self.nodes.retain(|_, r| {
            let keep = r.seen == generation;
            if !keep {
                drop_nodes(taffy, r);
            }
            keep
        });
        let available = taffy::Size {
            width: AvailableSpace::Definite(size.0),
            height: AvailableSpace::Definite(size.1),
        };
        let measured = self
            .taffy
            .compute_layout_with_measure(node, available, |inputs, _, measure, style| {
                taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, available| {
                    let Some(m) = measure else {
                        return taffy::Size::ZERO;
                    };
                    let (w, h) = m.size(text, known.width, available.width);
                    taffy::Size {
                        width: known.width.unwrap_or(w),
                        height: known.height.unwrap_or(h),
                    }
                })
            });
        if let Err(e) = measured {
            log::warn!("el layout: {e}");
        }
        // Absolute rects: each node's location is relative to its parent's.
        let mut origin: Vec<(f32, f32)> = Vec::with_capacity(order.len());
        let mut laid = Vec::with_capacity(order.len());
        // A scroll view's children hang from its content node.
        let mut stack: Vec<(NodeId, usize)> = Vec::new();
        for (i, &(id, node)) in order.iter().enumerate() {
            let parent = self.taffy.parent(node);
            while stack.last().is_some_and(|&(n, _)| Some(n) != parent) {
                stack.pop();
            }
            let at = stack.last().map(|&(_, i)| i);
            let l = self.taffy.layout(node).copied().unwrap_or_default();
            let base = at.map_or((0.0, 0.0), |p| origin[p]);
            let (x, y) = (base.0 + l.location.x, base.1 + l.location.y);
            let content = self.nodes.get(&id).and_then(|r| r.content.as_ref()).map(|(c, _)| *c);
            let inner = content.and_then(|c| self.taffy.layout(c).ok().copied());
            origin.push(inner.map_or((x, y), |c| (x + c.location.x, y + c.location.y)));
            laid.push(Laid {
                id,
                hit: hit_of(id),
                rect: Rect::new(x, y, l.size.width, l.size.height),
                parent: at,
                extent: inner.map(|c| (c.size.width, c.size.height)),
            });
            stack.push((content.unwrap_or(node), i));
        }
        laid
    }

    fn sync<S>(
        &mut self,
        el: &El<S>,
        id: u64,
        cx: &Cx,
        root: Option<(f32, f32)>,
        order: &mut Vec<(u64, NodeId)>,
    ) -> NodeId {
        let at = order.len();
        order.push((id, NodeId::new(0)));
        let mut ids: Vec<u64> = Vec::with_capacity(el.children.len());
        let children: Vec<NodeId> = el
            .children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                let mut child_id = child_id(id, child.key.as_deref(), i);
                if ids.contains(&child_id) {
                    log::warn!("el: duplicate key {:?}; using its place", child.key);
                    child_id = self::child_id(id, None, i);
                }
                ids.push(child_id);
                self.sync(child, child_id, cx, None, order)
            })
            .collect();
        let (style, content) = match el.scrolls() {
            Some(axis) => {
                let (viewport, content) = scroll_styles(style_of(el, cx, root), axis);
                (viewport, Some(content))
            }
            None => (style_of(el, cx, root), None),
        };
        let measure = measure_of(el, cx);
        let generation = self.generation;
        // An element that starts or stops scrolling is built again.
        if self.nodes.get(&id).is_some_and(|r| r.content.is_some() != content.is_some())
            && let Some(r) = self.nodes.remove(&id)
        {
            drop_nodes(&mut self.taffy, &r);
        }
        let node = match self.nodes.get_mut(&id) {
            Some(r) => {
                let mut touched = false;
                if r.style != style {
                    let _ = self.taffy.set_style(r.node, style.clone());
                    r.style = style;
                    touched = true;
                }
                if let (Some((node, old)), Some(new)) = (&mut r.content, content)
                    && *old != new
                {
                    let _ = self.taffy.set_style(*node, new.clone());
                    *old = new;
                    touched = true;
                }
                if r.measure != measure {
                    let _ = self.taffy.set_node_context(r.node, measure.clone());
                    r.measure = measure;
                    touched = true;
                }
                if r.children != children {
                    let parent = r.content.as_ref().map_or(r.node, |(c, _)| *c);
                    let _ = self.taffy.set_children(parent, &children);
                    r.children = children;
                    touched = true;
                }
                self.touched += usize::from(touched);
                r.seen = generation;
                r.node
            }
            None => {
                let made = "taffy nodes are created in memory";
                let (node, content) = match (&measure, content) {
                    (Some(m), _) => (self.taffy.new_leaf_with_context(style.clone(), m.clone()).expect(made), None),
                    (None, Some(inner)) => {
                        let c = self.taffy.new_with_children(inner.clone(), &children).expect(made);
                        (self.taffy.new_with_children(style.clone(), &[c]).expect(made), Some((c, inner)))
                    }
                    (None, None) => (self.taffy.new_with_children(style.clone(), &children).expect(made), None),
                };
                self.touched += 1;
                self.nodes.insert(
                    id,
                    Retained {
                        node,
                        style,
                        measure,
                        content,
                        children,
                        seen: generation,
                    },
                );
                node
            }
        };
        order[at].1 = node;
        node
    }
}

fn drop_nodes(taffy: &mut TaffyTree<Measure>, r: &Retained) {
    if let Some((content, _)) = &r.content {
        let _ = taffy.remove(*content);
    }
    let _ = taffy.remove(r.node);
}

/// A scroll view's own style (its box in its parent, clipping its one content child) and its
/// content node's: the element's flow, padding and gap, at its own length along `axis` (at
/// least the view's), stretched across.
fn scroll_styles(style: Style, axis: Axis) -> (Style, Style) {
    let mut content = Style {
        display: style.display,
        flex_direction: style.flex_direction,
        gap: style.gap,
        padding: style.padding,
        align_items: style.align_items,
        justify_content: style.justify_content,
        grid_template_columns: style.grid_template_columns.clone(),
        flex_shrink: 0.0,
        ..Style::default()
    };
    let (main, overflow) = match axis {
        Axis::Vertical => {
            content.min_size.height = LengthPercentageAuto::percent(1.0);
            (FlexDirection::Column, (Overflow::Hidden, Overflow::Scroll))
        }
        Axis::Horizontal => {
            content.min_size.width = LengthPercentageAuto::percent(1.0);
            (FlexDirection::Row, (Overflow::Scroll, Overflow::Hidden))
        }
    };
    let viewport = Style {
        display: Display::Flex,
        flex_direction: main,
        align_items: AlignItems::STRETCH,
        justify_content: JustifyContent::NORMAL,
        overflow: taffy::Point {
            x: overflow.0,
            y: overflow.1,
        },
        scrollbar_width: 0.0,
        padding: taffy::Rect::zero(),
        gap: taffy::Size::zero(),
        grid_template_columns: Vec::new(),
        ..style
    };
    (viewport, content)
}

impl Measure {
    /// The text's size in device px: one line as wide as it is, or wrapped at the width given.
    fn size(&self, text: &mut Text, known: Option<f32>, available: AvailableSpace) -> (f32, f32) {
        let (w, _) = text.measure(&self.text, &self.style);
        let Some(lines) = self.wrap else {
            return (w.ceil(), self.line);
        };
        let width = known.unwrap_or(match available {
            AvailableSpace::Definite(width) => width.min(w.ceil()),
            AvailableSpace::MaxContent => w.ceil(),
            AvailableSpace::MinContent => 0.0,
        });
        let wrapped = text.lines(&self.text, &self.style, width.max(1.0), lines);
        let widest = wrapped
            .iter()
            .map(|l| text.measure(l, &self.style).0)
            .fold(0.0, f32::max);
        (widest.ceil(), self.line * wrapped.len().max(1) as f32)
    }
}

fn measure_of<S>(el: &El<S>, cx: &Cx) -> Option<Measure> {
    let What::Text { text, role, wrap } = &el.what else {
        return None;
    };
    Some(Measure {
        text: text.clone(),
        style: cx.theme.font.style(*role, cx.s),
        line: cx.theme.font.line(*role, cx.s),
        wrap: *wrap,
    })
}

fn style_of<S>(el: &El<S>, cx: &Cx, root: Option<(f32, f32)>) -> Style {
    let z = &el.sizing;
    let len = |v: Len| LengthPercentage::length(cx.px(v));
    let auto_len = |v: Option<Len>| v.map_or(LengthPercentageAuto::auto(), |v| LengthPercentageAuto::length(cx.px(v)));
    let dim = |d: Option<Dim>| match d {
        None => Dimension::auto(),
        Some(Dim::Len(v)) => Dimension::length(cx.px(v)),
        Some(Dim::Full) => Dimension::percent(1.0),
    };
    let mut style = Style {
        display: Display::Flex,
        size: taffy::Size {
            width: dim(z.w),
            height: dim(z.h),
        },
        min_size: taffy::Size {
            width: auto_len(z.min_w),
            height: auto_len(z.min_h),
        },
        max_size: taffy::Size {
            width: auto_len(z.max_w),
            height: auto_len(z.max_h),
        },
        flex_grow: z.grow,
        padding: {
            let [top, right, bottom, left] = z.pad.map(|p| p.map_or(LengthPercentage::length(0.0), len));
            taffy::Rect {
                left,
                right,
                top,
                bottom,
            }
        },
        align_items: z.align.map_or(AlignItems::NORMAL, |a| match a {
            Align::Start => AlignItems::FLEX_START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::FLEX_END,
            Align::Stretch => AlignItems::STRETCH,
        }),
        justify_content: z.justify.map_or(JustifyContent::NORMAL, |j| match j {
            Justify::Start => JustifyContent::FLEX_START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::FLEX_END,
            Justify::Between => JustifyContent::SPACE_BETWEEN,
        }),
        ..Style::default()
    };
    if let Some(gap) = z.gap {
        style.gap = taffy::Size {
            width: len(gap),
            height: len(gap),
        };
    }
    if let Some((x, y)) = z.absolute {
        style.position = Position::Absolute;
        style.inset.left = LengthPercentageAuto::length(cx.px(x));
        style.inset.top = LengthPercentageAuto::length(cx.px(y));
    }
    match &el.what {
        What::Box(Flow::Row) => style.flex_direction = FlexDirection::Row,
        What::Box(Flow::Column) => style.flex_direction = FlexDirection::Column,
        What::Box(Flow::Grid(columns)) => {
            style.display = Display::Grid;
            style.grid_template_columns = vec![repeat(*columns, vec![fr(1.0)])];
        }
        // Labels shrink (cut with an ellipsis) rather than push their row wider.
        What::Text { .. } if z.min_w.is_none() => style.min_size.width = LengthPercentageAuto::length(0.0),
        What::Icon(_) | What::Arc(_) => {
            let side = Dimension::length(cx.px(token::ICON));
            style.size.width = z.w.map_or(side, |_| style.size.width);
            style.size.height = z.h.map_or(side, |_| style.size.height);
            style.flex_shrink = 0.0;
        }
        What::Image(image) => {
            if z.w.is_none() {
                style.size.width = Dimension::length(image.width as f32);
            }
            if z.h.is_none() {
                style.size.height = Dimension::length(image.height as f32);
            }
        }
        What::Text { .. } | What::Paint(_) => {}
    }
    // The root is the surface.
    if let Some((w, h)) = root {
        style.size = taffy::Size {
            width: Dimension::length(w),
            height: Dimension::length(h),
        };
        style.position = Position::Relative;
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el::{col, row, text};
    use crate::{Scheme, Style as Look, Ui};

    fn theme() -> Theme {
        Theme::new(Look::Liquid, Scheme::Dark, crate::theme::SYSTEM_BLUE)
    }

    #[test]
    fn a_row_places_fixed_and_growing_children_on_whole_pixels() {
        let mut ui = Ui::new().unwrap();
        let theme = theme();
        let cx = Cx { theme: &theme, s: 1.5 };
        let view = row::<()>()
            .pad(10.0)
            .gap(5.0)
            .child(col().w(20.0).h(10.0))
            .child(col().grow(1.0).h(10.0))
            .child(col().w(20.3).h(10.0));
        let laid = Layout::new().update(&view, (301.0, 100.0), &cx, &mut ui.text);
        let rects: Vec<Rect> = laid.iter().map(|l| l.rect).collect();
        assert_eq!(rects[0], Rect::new(0.0, 0.0, 301.0, 100.0));
        // 10 px pad and 5 gap at 1.5x: 15 and 8 (rounded like `Frame::px`).
        assert_eq!(rects[1], Rect::new(15.0, 15.0, 30.0, 15.0));
        assert_eq!(rects[3], Rect::new(301.0 - 15.0 - 30.0, 15.0, 30.0, 15.0));
        assert_eq!(rects[2].left, 15.0 + 30.0 + 8.0);
        assert_eq!(rects[2].right, rects[3].left - 8.0);
        for r in rects {
            for v in [r.left, r.top, r.right, r.bottom] {
                assert_eq!(v, v.round(), "{r:?} lands on whole pixels");
            }
        }
    }

    #[test]
    fn text_measures_through_the_cache_and_only_changed_nodes_are_touched() {
        let mut ui = Ui::new().unwrap();
        let theme = theme();
        let cx = Cx { theme: &theme, s: 1.0 };
        let view = |label: &str| {
            col::<()>()
                .align(crate::el::Align::Start)
                .child(text("Sound").key("title"))
                .child(row().child(text(label.to_string()).key("value")).child(col().w(4.0)))
        };
        let mut layout = Layout::new();
        let first = layout.update(&view("12%"), (200.0, 100.0), &cx, &mut ui.text);
        assert_eq!(layout.touched, 5, "everything is new");
        let style = theme.font.style(TextRole::Body, 1.0);
        let (w, _) = ui.text.measure("Sound", &style);
        assert_eq!(first[1].rect.width(), w.ceil());
        assert_eq!(first[1].rect.height(), theme.font.line(TextRole::Body, 1.0));

        layout.update(&view("12%"), (200.0, 100.0), &cx, &mut ui.text);
        assert_eq!(layout.touched, 0, "an identical view restyles nothing");
        let again = layout.update(&view("100%"), (200.0, 100.0), &cx, &mut ui.text);
        assert_eq!(layout.touched, 1, "only the changed text is re-measured");
        assert!(again[3].rect.width() > first[3].rect.width());
        assert_eq!(again[4].rect.left, again[3].rect.right, "its sibling moved along");
    }

    use crate::TextRole;
}
