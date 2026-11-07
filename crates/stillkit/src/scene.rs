//! What a view returns: a display list of paint operations plus the hit items (for input and
//! accessibility). A surface compares each new list with the one on screen and redraws only the
//! bounds of the operations that changed; an identical list costs nothing.

use std::sync::Arc;

pub(crate) const ARC_STEP: f32 = std::f32::consts::TAU / 10.0;

use accesskit::Role;

use crate::geom::{Point, Rect, Rgba};
use crate::text::TextStyle;

/// Identifies a hit item within its surface; the view chooses them.
pub type HitId = u32;

/// One paint operation, in the surface's device pixels, drawn in list order.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// A continuous corner surface; `width == 0` fills, otherwise strokes its contour.
    Continuous {
        /// Bounds in device pixels.
        rect: Rect,
        /// Radius.
        radius: f32,
        /// Smoothing.
        smoothing: f32,
        /// Width in device pixels.
        width: f32,
        /// Color.
        color: Rgba,
    },
    /// A clipped group, used by windowless scroll views. Bounds never escape the viewport.
    Clipped { #[doc = "Bounds in device pixels."] rect: Rect, #[doc = "Ops."] ops: Arc<[Op]> },
    /// An antialiased rounded clip for a composed card (picture, content and overlays).
    RoundedClipped { #[doc = "Bounds in device pixels."] rect: Rect, #[doc = "Radius."] radius: f32, #[doc = "Ops."] ops: Arc<[Op]> },
    /// Markdown emphasis without changing the shared text style contract.
    Emphasis {
        /// Bounds in device pixels.
        rect: Rect,
        /// Text content.
        text: Arc<str>,
        /// Style.
        style: TextStyle,
        /// Color.
        color: Rgba,
        /// Italic.
        italic: bool,
        /// Strike.
        strike: bool,
    },
    /// A clockwise circular arc of `sweep` radians, starting `start` radians clockwise from
    /// twelve o'clock (0: twelve, π/2: three...). Full turns draw an ellipse. Flat ends.
    Arc {
        /// Center.
        center: Point,
        /// Radius.
        radius: f32,
        /// Start, in radians clockwise from twelve o'clock.
        start: f32,
        /// Sweep, in radians clockwise from `start`.
        sweep: f32,
        /// Width in device pixels.
        width: f32,
        /// Color.
        color: Rgba,
    },
    /// An immutable premultiplied bitmap (photos, album artwork and app icons).
    Image {
        /// Bounds in device pixels.
        rect: Rect,
        /// Image.
        image: Arc<crate::Image>,
        /// Opacity.
        opacity: f32,
        /// Source.
        source: Option<Rect>,
    },
    /// [`Op::Image`] clipped to a rounded rectangle of `radius` (antialiased in one primitive:
    /// no specks at the corners), for pictures inside rounded cards and tiles.
    RoundedImage {
        /// Bounds in device pixels.
        rect: Rect,
        /// Image.
        image: Arc<crate::Image>,
        /// Opacity.
        opacity: f32,
        /// Source.
        source: Option<Rect>,
        /// Radius.
        radius: f32,
    },
    /// A snapshot of a field's revision, so edits invalidate its bounds without an idle tick.
    TextField {
        /// Bounds in device pixels.
        rect: Rect,
        /// Field.
        field: crate::TextField,
        /// Revision.
        revision: u64,
    },
    /// A filled rectangle, rounded when `radius > 0`.
    Fill {
        /// Bounds in device pixels.
        rect: Rect,
        /// Radius.
        radius: f32,
        /// Color.
        color: Rgba,
    },
    /// A rectangle's outline, centred on its edge.
    Stroke {
        /// Bounds in device pixels.
        rect: Rect,
        /// Radius.
        radius: f32,
        /// Width in device pixels.
        width: f32,
        /// Color.
        color: Rgba,
    },
    /// Line.
    Line {
        /// From.
        from: Point,
        /// To.
        to: Point,
        /// Width in device pixels.
        width: f32,
        /// Color.
        color: Rgba,
    },
    /// A string (or an icon glyph in an icon font) with its top-left at `rect`'s; `rect` is its
    /// measured size ([`crate::Text::measure`]).
    Text {
        /// Bounds in device pixels.
        rect: Rect,
        /// Text content.
        text: Arc<str>,
        /// Style.
        style: TextStyle,
        /// Color.
        color: Rgba,
    },
    /// A vector icon ([`crate::icons`]) filling the square `rect`, stroked `weight` device
    /// pixels wide with round caps and joins (and filled too for `Icon::filled` ones).
    Icon {
        /// Bounds in device pixels.
        rect: Rect,
        /// Icon.
        icon: crate::icons::Icon,
        /// Weight.
        weight: f32,
        /// Color.
        color: Rgba,
    },
    /// A stepped shadow under `rect`: fading rings `spread` pixels out, moved down by `drop`.
    /// The immediate-mode kit's; new code draws [`Op::SoftShadow`].
    Shadow {
        /// Bounds in device pixels.
        rect: Rect,
        /// Radius.
        radius: f32,
        /// Spread.
        spread: f32,
        /// Drop.
        drop: f32,
        /// Color.
        color: Rgba,
    },
    /// A real soft shadow with CSS `box-shadow` semantics (outset): the rounded rectangle
    /// `rect` (corner `radius`) grown by `spread` on every side (its corners by `spread` too,
    /// never below 0), moved by `offset`, filled with `color` and blurred by a Gaussian of
    /// σ = `blur / 2`. So `color` is the shadow's peak opacity, reached under a shape larger
    /// than its blur (half of it on a straight edge); it is zero past 3σ outside the grown
    /// shape. It paints under the shape too (draw the shape over it).
    ///
    /// Windows draws it from an analytic, cached 9-slice mask (CPU and GPU alike), Linux with
    /// Vello's blurred rounded rectangle; a macOS backend can use CoreGraphics' shadow
    /// (`CGContextSetShadowWithColor` with `offset`, a blur of `blur` and `color`) under the
    /// grown shape, drawn outside the clip so only its shadow lands.
    SoftShadow {
        /// Bounds in device pixels.
        rect: Rect,
        /// Radius.
        radius: f32,
        /// Blur.
        blur: f32,
        /// Offset.
        offset: (f32, f32),
        /// Spread.
        spread: f32,
        /// Color.
        color: Rgba,
    },
    /// Clips what follows to `rect` (intersected with any clip already pushed) until the
    /// matching [`Op::Unclip`]. Scroll views wrap their content in a pair.
    Clip { #[doc = "Bounds in device pixels."] rect: Rect },
    /// Ends the innermost [`Op::Clip`].
    Unclip,
}

impl Op {
    /// Every pixel this operation may touch, antialiasing included.
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Continuous { rect, width, .. } => rect.inflate(width * 0.5 + 1.0),
            Self::Clipped { rect, .. } | Self::RoundedClipped { rect, .. } => *rect,
            Self::Emphasis { rect, style, .. } => rect.inflate(2.0 + style.size * 0.1),
            Self::Arc {
                center,
                radius,
                width,
                ..
            } => Rect::new(
                center.x - radius,
                center.y - radius,
                radius * 2.0,
                radius * 2.0,
            )
            .inflate(width / 2.0 + 1.0),
            Self::Image { rect, .. }
            | Self::RoundedImage { rect, .. }
            | Self::TextField { rect, .. } => rect.inflate(1.0),
            Self::Fill { rect, .. } => rect.inflate(1.0),
            Self::Icon { rect, weight, .. } => rect.inflate(weight + 1.0),
            Self::Stroke { rect, width, .. } => rect.inflate(width / 2.0 + 1.0),
            Self::Line {
                from, to, width, ..
            } => Rect {
                left: from.x.min(to.x),
                top: from.y.min(to.y),
                right: from.x.max(to.x),
                bottom: from.y.max(to.y),
            }
            .inflate(width + 1.0),
            // Glyphs may overhang their layout box a little.
            Self::Text { rect, style, .. } => rect.inflate(2.0 + style.size * 0.1),
            Self::Shadow {
                rect, spread, drop, ..
            } => rect.inflate(spread + 1.0).offset(0.0, *drop),
            // Its reach past the grown shape, plus the pixel its edges snap by.
            Self::SoftShadow {
                rect,
                radius,
                blur,
                offset,
                spread,
                ..
            } => {
                let (shape, _) = crate::shadow::shape(*rect, *radius, *offset, *spread);
                shape.inflate(crate::shadow::REACH * crate::shadow::sigma(*blur) + 2.0)
            }
            // A changed clip changes what everything inside it may show.
            Self::Clip { rect } => *rect,
            Self::Unclip => Rect::default(),
        }
    }
}

/// Something the pointer, the keyboard and assistive technology can land on.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Stable identity used to match this item across frames.
    pub id: HitId,
    /// Bounds in device pixels.
    pub rect: Rect,
    /// Role.
    pub role: Role,
    /// Text announced to assistive technology.
    pub label: Arc<str>,
    /// Disabled items are still announced (as disabled), but never selected.
    pub enabled: bool,
    /// It does something: only actionable items take hover, press, focus and activation, and
    /// offer assistive technology their actions. A label with a role is enabled but not
    /// actionable; a disabled control is neither.
    pub actionable: bool,
    /// For check items: whether the mark is on.
    pub checked: Option<bool>,
}

impl Hit {
    /// A hit item that is actionable exactly when it is enabled (every control). Set
    /// `actionable: false` on an enabled one for plain announced content.
    pub fn new(
        id: HitId,
        rect: Rect,
        role: Role,
        label: impl Into<Arc<str>>,
        enabled: bool,
        checked: Option<bool>,
    ) -> Self {
        Self {
            id,
            rect,
            role,
            label: label.into(),
            enabled,
            actionable: enabled,
            checked,
        }
    }
}

/// One frame of a surface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayList {
    /// Ops.
    pub ops: Vec<Op>,
    /// Hit items, topmost last.
    pub hits: Vec<Hit>,
    /// The hit item with keyboard focus.
    pub focus: Option<HitId>,
}

impl DisplayList {
    /// Moves a list into a cropped surface without changing its hit identities.
    pub fn translated(&self, dx: f32, dy: f32) -> Self {
        let mut list = self.clone();
        fn translate(ops: &mut [Op], dx: f32, dy: f32) {
            for op in ops {
                match op {
                    Op::Clipped { rect, ops } | Op::RoundedClipped { rect, ops, .. } => {
                        *rect = rect.offset(dx, dy);
                        translate(Arc::make_mut(ops), dx, dy);
                    }
                    Op::Arc { center, .. } => {
                        center.x += dx;
                        center.y += dy;
                    }
                    Op::Line { from, to, .. } => {
                        from.x += dx;
                        from.y += dy;
                        to.x += dx;
                        to.y += dy;
                    }
                    Op::Continuous { rect, .. }
                    | Op::Image { rect, .. }
                    | Op::RoundedImage { rect, .. }
                    | Op::TextField { rect, .. }
                    | Op::Fill { rect, .. }
                    | Op::Icon { rect, .. }
                    | Op::Stroke { rect, .. }
                    | Op::Text { rect, .. }
                    | Op::Emphasis { rect, .. }
                    | Op::Clip { rect }
                    | Op::Shadow { rect, .. }
                    | Op::SoftShadow { rect, .. } => *rect = rect.offset(dx, dy),
                    Op::Unclip => {}
                }
            }
        }
        translate(&mut list.ops, dx, dy);
        for hit in &mut list.hits {
            hit.rect = hit.rect.offset(dx, dy);
        }
        list
    }

    /// The topmost hit item under `p`.
    pub fn hit_at(&self, p: Point) -> Option<&Hit> {
        self.hits.iter().rev().find(|h| h.rect.contains(p))
    }

    /// What has to be redrawn to go from `old` to this list: the bounds of every operation that
    /// differs (position by position), or `None` when they paint the same.
    pub fn damage(&self, old: &DisplayList) -> Option<Rect> {
        damage_ops(&old.ops, &self.ops)
    }

    /// Disjoint damage for dashboards: a CPU and network change need not redraw the labels
    /// or meters between them. Layered-window presentation still happens just once.
    pub fn damage_regions(&self, old: &DisplayList) -> Vec<Rect> {
        let mut regions = Vec::new();
        collect_damage(&old.ops, &self.ops, None, &mut regions);
        let mut merged: Vec<Rect> = Vec::new();
        for mut rect in regions {
            while let Some(index) = merged
                .iter()
                .position(|old| old.inflate(1.0).intersects(&rect))
            {
                rect = rect.union(merged.swap_remove(index));
            }
            merged.push(rect);
            if merged.len() > 64 {
                return self.damage(old).into_iter().collect();
            }
        }
        merged
    }
}

fn collect_damage(old: &[Op], new: &[Op], clip: Option<Rect>, out: &mut Vec<Rect>) {
    for i in 0..old.len().max(new.len()) {
        let changed = match (old.get(i), new.get(i)) {
            (Some(a), Some(b)) if a == b => continue,
            (
                Some(Op::Clipped {
                    rect: a,
                    ops: before,
                }),
                Some(Op::Clipped {
                    rect: b,
                    ops: after,
                }),
            ) if a == b => {
                let clip = clip.map_or(*a, |outer| intersection(outer, *a));
                collect_damage(before, after, Some(clip), out);
                continue;
            }
            (Some(a), Some(b)) => arc_change(a, b).unwrap_or_else(|| a.bounds().union(b.bounds())),
            (Some(one), None) | (None, Some(one)) => one.bounds(),
            (None, None) => continue,
        };
        let changed = clip.map_or(changed, |clip| intersection(changed, clip));
        if !changed.is_empty() {
            out.push(changed);
        }
    }
}

/// Fixed tenth-turn prefixes keep unchanged pixels stable. The final partial segment can
/// change its curve approximation back to its start, so damage that complete segment.
fn arc_change(old: &Op, new: &Op) -> Option<Rect> {
    let (
        Op::Arc {
            center: a,
            radius: ar,
            start: from,
            sweep: before,
            width: aw,
            color: ac,
        },
        Op::Arc {
            center: b,
            radius: br,
            start: to,
            sweep: after,
            width: bw,
            color: bc,
        },
    ) = (old, new)
    else {
        return None;
    };
    if a != b
        || from != to
        || ar != br
        || aw != bw
        || ac != bc
        || !before.is_finite()
        || !after.is_finite()
        || *before >= std::f32::consts::TAU - 0.0001
        || *after >= std::f32::consts::TAU - 0.0001
    {
        return None;
    }
    let start = (before.min(*after).max(0.0) / ARC_STEP).floor() * ARC_STEP;
    let end =
        ((before.max(*after).max(0.0) / ARC_STEP).ceil() * ARC_STEP).min(std::f32::consts::TAU);
    // Angles from twelve o'clock: the changed stretch runs from `from + start` to `from + end`,
    // and reaches out as far as the quarter points inside it.
    let from = from.rem_euclid(std::f32::consts::TAU);
    let (start, end) = (from + start, from + end);
    let point = |angle: f32| Point::new(a.x + ar * angle.sin(), a.y - ar * angle.cos());
    let first = point(start);
    let mut rect = Rect::new(first.x, first.y, 0.0, 0.0);
    let quarters = (0..=12).map(|k| k as f32 * std::f32::consts::FRAC_PI_2 - std::f32::consts::TAU);
    for angle in std::iter::once(end).chain(quarters) {
        if angle >= start && angle <= end {
            let p = point(angle);
            rect.left = rect.left.min(p.x);
            rect.right = rect.right.max(p.x);
            rect.top = rect.top.min(p.y);
            rect.bottom = rect.bottom.max(p.y);
        }
    }
    Some(rect.inflate(aw / 2.0 + 2.0))
}
fn intersection(a: Rect, b: Rect) -> Rect {
    Rect {
        left: a.left.max(b.left),
        top: a.top.max(b.top),
        right: a.right.min(b.right),
        bottom: a.bottom.min(b.bottom),
    }
}

fn damage_ops(old: &[Op], new: &[Op]) -> Option<Rect> {
    let count = new.len().max(old.len());
    (0..count)
        .filter_map(|i| match (old.get(i), new.get(i)) {
            (Some(a), Some(b)) if a == b => None,
            (
                Some(Op::Clipped {
                    rect: a,
                    ops: before,
                }),
                Some(Op::Clipped {
                    rect: b,
                    ops: after,
                }),
            ) if a == b => {
                let changed = damage_ops(before, after)?;
                let clipped = Rect {
                    left: changed.left.max(a.left),
                    top: changed.top.max(a.top),
                    right: changed.right.min(a.right),
                    bottom: changed.bottom.min(a.bottom),
                };
                (!clipped.is_empty()).then_some(clipped)
            }
            (a, b) => {
                let bounds = |op: Option<&Op>| op.map(Op::bounds).filter(|r| !r.is_empty());
                match (bounds(a), bounds(b)) {
                    (Some(a), Some(b)) => Some(a.union(b)),
                    (one, other) => one.or(other),
                }
            }
        })
        .reduce(Rect::union)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(x: f32) -> Op {
        Op::Fill {
            rect: Rect::new(x, 0.0, 10.0, 10.0),
            radius: 0.0,
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
        }
    }

    fn list(ops: Vec<Op>) -> DisplayList {
        DisplayList {
            ops,
            ..Default::default()
        }
    }

    #[test]
    fn identical_lists_need_no_redraw_whatever_their_hits_say() {
        let a = list(vec![fill(0.0), fill(20.0)]);
        let mut b = a.clone();
        b.focus = Some(3);
        assert_eq!(b.damage(&a), None);
    }

    #[test]
    fn a_changed_scrolled_meter_damages_its_own_bounds_and_not_the_whole_viewport() {
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before = list(vec![Op::Clipped {
            rect: viewport,
            ops: vec![fill(0.0), fill(100.0), fill(300.0)].into(),
        }]);
        let after = list(vec![Op::Clipped {
            rect: viewport,
            ops: vec![fill(0.0), fill(105.0), fill(300.0)].into(),
        }]);
        assert_eq!(
            after.damage(&before),
            Some(Rect::new(99.0, 0.0, 17.0, 11.0))
        );
        let outside = list(vec![Op::Clipped {
            rect: viewport,
            ops: vec![fill(0.0), fill(100.0), fill(500.0)].into(),
        }]);
        let changed_outside = list(vec![Op::Clipped {
            rect: viewport,
            ops: vec![fill(0.0), fill(100.0), fill(510.0)].into(),
        }]);
        assert_eq!(changed_outside.damage(&outside), None);
    }

    #[test]
    fn meter_sweep_damage_keeps_the_unchanged_ring() {
        let arc = |sweep| Op::Arc {
            center: Point::new(50.0, 50.0),
            radius: 40.0,
            start: 0.0,
            sweep,
            width: 3.0,
            color: Rgba::HIT,
        };
        let before = list(vec![arc(0.20 * std::f32::consts::TAU)]);
        let after = list(vec![arc(0.21 * std::f32::consts::TAU)]);
        let regions = after.damage_regions(&before);
        assert_eq!(regions.len(), 1);
        assert!(regions[0].width() < 20.0 && regions[0].height() < 40.0);
        assert!(regions[0].left > 80.0 && regions[0].top > 30.0);
        assert_eq!(before.damage_regions(&after), regions);

        // From 7:30 the same growth lands up and to the left of the centre (past 9 o'clock).
        let from = |sweep| Op::Arc {
            center: Point::new(50.0, 50.0),
            radius: 40.0,
            start: 1.25 * std::f32::consts::PI,
            sweep,
            width: 3.0,
            color: Rgba::HIT,
        };
        let before = list(vec![from(0.20 * std::f32::consts::TAU)]);
        let after = list(vec![from(0.21 * std::f32::consts::TAU)]);
        let regions = after.damage_regions(&before);
        assert_eq!(regions.len(), 1);
        assert!(regions[0].right < 50.0 && regions[0].bottom < 50.0, "{:?}", regions[0]);
        assert!(regions[0].width() < 40.0 && regions[0].height() < 40.0);
    }

    #[test]
    fn sparse_damage_keeps_distant_changes_separate_and_merges_neighbors() {
        let before = list(vec![fill(0.0), fill(15.0), fill(100.0)]);
        let after = list(vec![fill(4.0), fill(17.0), fill(110.0)]);
        let mut regions = after.damage_regions(&before);
        regions.sort_by(|a, b| a.left.total_cmp(&b.left));
        assert_eq!(
            regions,
            vec![
                Rect::new(-1.0, -1.0, 29.0, 12.0),
                Rect::new(99.0, -1.0, 22.0, 12.0),
            ]
        );
        assert!(before.damage_regions(&before).is_empty());
    }

    #[test]
    fn a_moving_soft_shadow_redraws_its_old_and_new_bounds_only() {
        let panel = |x: f32| {
            let rect = Rect::new(x, 40.0, 300.0, 400.0);
            vec![
                Op::SoftShadow {
                    rect,
                    radius: 12.0,
                    blur: 24.0,
                    offset: (0.0, 8.0),
                    spread: 2.0,
                    color: Rgba::new(0.0, 0.0, 0.0, 0.3),
                },
                Op::Fill { rect, radius: 12.0, color: Rgba::new(1.0, 1.0, 1.0, 1.0) },
            ]
        };
        let still = fill(2000.0);
        let before = list([panel(100.0), vec![still.clone()]].concat());
        let after = list([panel(112.0), vec![still]].concat());
        let (old, new) = (before.ops[0].bounds(), after.ops[0].bounds());
        // The shadow's old and new place (the panel lies inside them); the still fill stays.
        assert_eq!(after.damage_regions(&before), vec![old.union(new)]);
        // The shadow reaches 3σ (36 px) past its grown, offset shape, and no further.
        assert_eq!(old, Rect::new(100.0 - 2.0 - 38.0, 40.0 + 8.0 - 2.0 - 38.0, 380.0, 480.0));
        assert_eq!(after.damage(&after), None);
    }

    #[test]
    fn damage_covers_only_the_operations_that_changed() {
        let a = list(vec![fill(0.0), fill(20.0), fill(40.0)]);
        let b = list(vec![fill(0.0), fill(25.0), fill(40.0)]);
        // The old and the new place of the moved fill, nothing else.
        assert_eq!(b.damage(&a), Some(Rect::new(19.0, -1.0, 17.0, 12.0)));
        // A removed operation is damage where it was.
        let shorter = list(vec![fill(0.0)]);
        let gone = shorter.damage(&a).unwrap();
        assert!(gone.left <= 19.0 && gone.right >= 51.0);
        assert!(gone.left > 10.0, "the kept fill is not redrawn");
    }
}
