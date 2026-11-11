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
    /// Filters an isolated completed subtree.
    Blurred {
        /// Gaussian standard deviation in device pixels, capped at 64.
        sigma: f32,
        /// Subtree paint.
        ops: Arc<[Op]>,
    },
    /// Filters preceding paint in the same surface through a rounded mask.
    BackdropBlur {
        /// Panel bounds.
        rect: Rect,
        /// Corner radius.
        radius: f32,
        /// Gaussian standard deviation in device pixels, capped at 64.
        sigma: f32,
        /// Strength, including inherited opacity.
        opacity: f32,
    },
    /// Draws a subtree in an affine coordinate system, including clips and text.
    Transformed {
        /// Local coordinates to the enclosing coordinate system.
        transform: crate::Transform,
        /// Local operations.
        ops: Arc<[Op]>,
    },
    /// A native gradient on a rounded rectangle; zero width fills, positive width strokes.
    Gradient {
        /// Bounds in device pixels.
        rect: Rect,
        /// Corner radius.
        radius: f32,
        /// Stroke width, or zero for a fill.
        width: f32,
        /// Stops and direction.
        gradient: crate::paint::Gradient,
    },
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
    /// Italic or struck-through text, used by attributed display runs.
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
    /// Clears `rect` to transparent: a hole that shows what lies beneath the surface, such as
    /// a native GPU layer ([`crate::el::native`]). Drawn in order, so later ops paint over it.
    Hole { #[doc = "Bounds in device pixels."] rect: Rect },
    /// Clips what follows to `rect` (intersected with any clip already pushed) until the
    /// matching [`Op::Unclip`]. Scroll views wrap their content in a pair.
    Clip { #[doc = "Bounds in device pixels."] rect: Rect },
    /// Ends the innermost [`Op::Clip`].
    Unclip,
}

impl Op {
    /// Moves it by `(dx, dy)` device px, nested ops included. Hosts that place one list
    /// inside another use this (or [`DisplayList::translated`]) instead of matching every
    /// variant, so a new op never breaks them.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Op::Blurred { ops, .. } => { for op in Arc::make_mut(ops) { op.translate(dx, dy); } }
            Op::BackdropBlur { rect, .. } => *rect = rect.offset(dx, dy),
            Op::Transformed { transform, .. } => *transform = crate::Transform::new(Point::new(0.,0.),0.,(1.,1.),(dx,dy)).compose(*transform),
            Op::Clipped { rect, ops } | Op::RoundedClipped { rect, ops, .. } => {
                *rect = rect.offset(dx, dy);
                for op in Arc::make_mut(ops) {
                    op.translate(dx, dy);
                }
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
            Op::Gradient { rect, .. }
            | Op::Continuous { rect, .. }
            | Op::Image { rect, .. }
            | Op::RoundedImage { rect, .. }
            | Op::TextField { rect, .. }
            | Op::Fill { rect, .. }
            | Op::Icon { rect, .. }
            | Op::Stroke { rect, .. }
            | Op::Text { rect, .. }
            | Op::Emphasis { rect, .. }
            | Op::Clip { rect }
            | Op::Hole { rect }
            | Op::Shadow { rect, .. }
            | Op::SoftShadow { rect, .. } => *rect = rect.offset(dx, dy),
            Op::Unclip => {}
        }
    }

    /// Scales it by `by` about the origin: positions, sizes, radii, widths, blurs and text
    /// sizes, nested ops included. A host that lays out at one size and paints at another
    /// (a thumbnail) uses this instead of matching every variant.
    pub fn scale(&mut self, by: f32) {
        let rect = |r: &mut Rect| {
            r.left *= by;
            r.top *= by;
            r.right *= by;
            r.bottom *= by;
        };
        let point = |p: &mut Point| {
            p.x *= by;
            p.y *= by;
        };
        match self {
            Op::Transformed { transform, .. } => *transform = crate::Transform::new(Point::new(0.,0.),0.,(by,by),(0.,0.)).compose(*transform),
            Op::Blurred { sigma, ops } => { *sigma *= by; for op in Arc::make_mut(ops) { op.scale(by); } }
            Op::BackdropBlur { rect: r, radius, sigma, .. } => { rect(r); *radius *= by; *sigma *= by; }
            Op::Image { rect: r, .. } | Op::Clip { rect: r } | Op::Hole { rect: r } | Op::TextField { rect: r, .. } => rect(r),
            Op::RoundedImage { rect: r, radius, .. } | Op::Fill { rect: r, radius, .. } => {
                rect(r);
                *radius *= by;
            }
            Op::Stroke { rect: r, radius, width, .. }
            | Op::Continuous { rect: r, radius, width, .. }
            | Op::Gradient { rect: r, radius, width, .. } => {
                rect(r);
                *radius *= by;
                *width *= by;
            }
            Op::Icon { rect: r, weight, .. } => {
                rect(r);
                *weight *= by;
            }
            Op::Text { rect: r, style, .. } | Op::Emphasis { rect: r, style, .. } => {
                rect(r);
                style.size *= by;
                style.tracking *= by;
            }
            Op::Line { from, to, width, .. } => {
                point(from);
                point(to);
                *width *= by;
            }
            Op::Arc { center, radius, width, .. } => {
                point(center);
                *radius *= by;
                *width *= by;
            }
            Op::Shadow { rect: r, radius, spread, drop, .. } => {
                rect(r);
                *radius *= by;
                *spread *= by;
                *drop *= by;
            }
            Op::SoftShadow { rect: r, radius, blur, offset, spread, .. } => {
                rect(r);
                *radius *= by;
                *blur *= by;
                offset.0 *= by;
                offset.1 *= by;
                *spread *= by;
            }
            Op::Clipped { rect: r, ops } => {
                rect(r);
                for op in Arc::make_mut(ops) {
                    op.scale(by);
                }
            }
            Op::RoundedClipped { rect: r, radius, ops } => {
                rect(r);
                *radius *= by;
                for op in Arc::make_mut(ops) {
                    op.scale(by);
                }
            }
            Op::Unclip => {}
        }
    }

    /// Native-layer holes in `ops` ([`Op::Hole`]), in drawing order, as device rects cut to
    /// the clips around them: what a renderer that can't erase paint (a GPU one without
    /// destructive blends) clips out of the paint drawn before each hole instead.
    pub fn holes(ops: &[Op]) -> Vec<Rect> {
        fn walk(ops: &[Op], clip: Rect, map: Option<crate::Transform>, out: &mut Vec<Rect>) {
            for op in ops {
                match op {
                    Op::Hole { rect } => {
                        let rect = map.map_or(*rect, |t| t.bounds(*rect));
                        out.push(rect.intersect(clip));
                    }
                    Op::Clipped { rect, ops } | Op::RoundedClipped { rect, ops, .. } => {
                        let rect = map.map_or(*rect, |t| t.bounds(*rect));
                        walk(ops, clip.intersect(rect), map, out);
                    }
                    Op::Transformed { transform, ops } => {
                        let map = Some(map.map_or(*transform, |m| m.compose(*transform)));
                        walk(ops, clip, map, out);
                    }
                    Op::Blurred { ops, .. } => walk(ops, clip, map, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(ops, Rect::new(-32768.0, -32768.0, 65536.0, 65536.0), None, &mut out);
        out
    }

    /// Whether this op draws paint itself, rather than grouping ops, clipping or cutting.
    pub fn is_paint(&self) -> bool {
        !matches!(
            self,
            Op::Blurred { .. }
                | Op::BackdropBlur { .. }
                | Op::Transformed { .. }
                | Op::Clipped { .. }
                | Op::RoundedClipped { .. }
                | Op::Hole { .. }
                | Op::Clip { .. }
                | Op::Unclip
        )
    }

    /// Every pixel this operation may touch, antialiasing included.
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Blurred { sigma, ops } => ops.iter().map(Op::bounds).reduce(Rect::union).unwrap_or_default().inflate(crate::blur::reach(*sigma)),
            Self::BackdropBlur { rect, .. } => *rect,
            Self::Transformed { transform, ops } => ops.iter().map(Op::bounds).reduce(|a,b|a.union(b)).map(|r|transform.bounds(r).inflate(1.0)).unwrap_or_default(),
            Self::Gradient { rect, width, .. } | Self::Continuous { rect, width, .. } => rect.inflate(width * 0.5 + 1.0),
            Self::Clipped { rect, .. } => *rect,
            Self::RoundedClipped { rect, ops, .. } => ops.iter()
                .map(Op::bounds)
                .reduce(|a, b| a.union(b))
                .unwrap_or_default()
                .intersect(*rect),
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
            Self::Hole { rect } => *rect,
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
    /// Precise transformed geometry; absent for ordinary rectangular hits.
    pub shape: Option<Box<HitShape>>,
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

/// A transformed hit region, retaining clips at each enclosing scope.
#[derive(Debug, Clone, PartialEq)]
pub struct HitShape {
    rect: Rect,
    transform: crate::Transform,
    inner: Option<Box<HitShape>>,
}
impl HitShape {
    fn unmap(&self,p:Point)->Point {
        let p=self.transform.inverse().map_or(p,|xf|xf.point(p));
        self.inner.as_ref().map_or(p,|s|s.unmap(p))
    }
    fn contains(&self,p:Point)->bool {
        self.transform.inverse().is_some_and(|xf| { let p=xf.point(p); self.rect.contains(p) && self.inner.as_ref().is_none_or(|s|s.contains(p)) })
    }
}
impl Hit {
    /// Applies a draw-time transform, retaining the original hit region.
    pub fn transform(&mut self, transform: crate::Transform) {
        self.shape = Some(Box::new(HitShape { rect:self.rect,transform,inner:self.shape.take() }));
        self.rect = transform.bounds(self.rect);
    }
    /// Maps a surface point back through the element's draw-time transforms.
    pub fn unmap(&self,p:Point)->Point { self.shape.as_ref().map_or(p,|s|s.unmap(p)) }
    /// Tests the painted region, including enclosing clips and transforms.
    pub fn contains(&self,p:Point)->bool { self.rect.contains(p) && self.shape.as_ref().is_none_or(|s|s.contains(p)) }

    /// A hit item that is actionable exactly when it is enabled (every control). Set
    /// `actionable: false` on an enabled one for plain announced content. Hosts build hits
    /// with this rather than a struct literal, so a new field never breaks them.
    pub fn new(
        id: HitId,
        rect: Rect,
        role: Role,
        label: impl Into<Arc<str>>,
        enabled: bool,
        checked: Option<bool>,
    ) -> Self {
        Self {
            shape: None,
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
        for op in &mut list.ops {
            op.translate(dx, dy);
        }
        for hit in &mut list.hits {
            if hit.shape.is_some() { hit.transform(crate::Transform::new(Point::new(0.,0.),0.,(1.,1.),(dx,dy))); }
            else { hit.rect = hit.rect.offset(dx, dy); }
        }
        list
    }

    /// The list drawn `k` times as large from its origin (one transform around every op, so
    /// text and paths render sharp at the new size), its hit items scaled with it.
    pub fn scaled(&self, k: f32) -> Self {
        let transform = crate::Transform([k, 0., 0., k, 0., 0.]);
        let mut list = self.clone();
        if (k - 1.0).abs() < f32::EPSILON {
            return list;
        }
        list.ops = vec![Op::Transformed { transform, ops: std::mem::take(&mut list.ops).into() }];
        for hit in &mut list.hits {
            if hit.shape.is_some() { hit.transform(transform); }
            else {
                let r = hit.rect;
                hit.rect = Rect::new(r.left * k, r.top * k, r.width() * k, r.height() * k);
            }
        }
        list
    }

    /// The topmost hit item under `p`.
    pub fn hit_at(&self, p: Point) -> Option<&Hit> {
        self.hits.iter().rev().find(|h| h.contains(p))
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
        backdrop_damage(&self.ops, &mut regions);
        backdrop_damage(&old.ops, &mut regions);
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

/// The changed middle of two op lists: their equal start and end trimmed, so an op inserted
/// or removed damages only what actually changed, not every op after it.
fn changed_span<'a>(old: &'a [Op], new: &'a [Op]) -> (&'a [Op], &'a [Op]) {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (&old[prefix..], &new[prefix..]);
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (&old[..old.len() - suffix], &new[..new.len() - suffix])
}

/// How far ahead a mismatch looks for the lists to line up again (an inserted or removed
/// run of up to this many ops costs only itself).
const RESYNC: usize = 24;

fn collect_damage(old: &[Op], new: &[Op], clip: Option<Rect>, out: &mut Vec<Rect>) {
    let (old, new) = changed_span(old, new);
    // A changed rect, kept to the clip it draws in.
    fn push(out: &mut Vec<Rect>, clip: Option<Rect>, rect: Rect) {
        let rect = clip.map_or(rect, |clip| intersection(rect, clip));
        if !rect.is_empty() {
            out.push(rect);
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) if a == b => {
                i += 1;
                j += 1;
            }
            (Some(Op::Blurred {sigma:a,ops:before}), Some(Op::Blurred {sigma:b,ops:after})) if a == b => {
                let mut inner=Vec::new();
                collect_damage(before,after,None,&mut inner);
                for rect in inner {push(out,clip,rect.inflate(crate::blur::reach(*a)));}
                i+=1; j+=1;
            }
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
                let inner = clip.map_or(*a, |outer| intersection(outer, *a));
                collect_damage(before, after, Some(inner), out);
                i += 1;
                j += 1;
            }
            (Some(a), Some(b)) => {
                // Ops inserted (or removed) here: the lists line up again a few ops on. A run
                // never skips a clip boundary: ops past a moved Clip/Unclip draw under another
                // clip even when they are equal, and Unclip has no bounds to damage.
                let clean = |run: &[Op]| !run.iter().any(is_clip_boundary);
                let inserted = (1..=RESYNC).find(|&k| new.get(j + k) == Some(a) && clean(&new[j..j + k]));
                let removed = (1..=RESYNC).find(|&k| old.get(i + k) == Some(b) && clean(&old[i..i + k]));
                if inserted.is_none() && removed.is_none() && (is_clip_boundary(a) || is_clip_boundary(b)) {
                    // A clip boundary changed: everything after it may draw under a new clip.
                    old[i..].iter().chain(&new[j..]).for_each(|op| push(out, clip, op.bounds()));
                    break;
                }
                match (inserted, removed) {
                    (Some(k), r) if r.is_none_or(|r| k <= r) => {
                        new[j..j + k].iter().for_each(|op| push(out, clip, op.bounds()));
                        j += k;
                    }
                    (_, Some(k)) => {
                        old[i..i + k].iter().for_each(|op| push(out, clip, op.bounds()));
                        i += k;
                    }
                    _ => {
                        // Changed in place: where it was and where it is (apart, if it moved).
                        match arc_change(a, b) {
                            Some(arc) => push(out, clip, arc),
                            None => {
                                push(out, clip, a.bounds());
                                push(out, clip, b.bounds());
                            }
                        }
                        i += 1;
                        j += 1;
                    }
                }
            }
            (Some(one), None) => {
                push(out, clip, one.bounds());
                i += 1;
            }
            (None, Some(one)) => {
                push(out, clip, one.bounds());
                j += 1;
            }
            (None, None) => break,
        }
    }
}

/// Whether `op` opens or closes a clip that later ops draw under.
fn is_clip_boundary(op: &Op) -> bool {
    matches!(op, Op::Clip { .. } | Op::Unclip)
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

// A filter reads beyond its output. Damage propagates through later backdrop filters.
fn backdrop_damage(ops: &[Op], regions: &mut Vec<Rect>) {
    for op in ops {
        match op {
            Op::BackdropBlur { rect, sigma, .. } => {
                let input = rect.inflate(crate::blur::reach(*sigma));
                let affected: Vec<_> = regions.iter().filter(|r| r.intersects(&input))
                    .map(|r| r.inflate(crate::blur::reach(*sigma)).intersect(*rect))
                    .filter(|r| !r.is_empty()).collect();
                regions.extend(affected);
            }
            Op::Clipped { rect, ops } | Op::RoundedClipped { rect, ops, .. } => {
                let mut inner=regions.clone();
                let before=inner.len();
                backdrop_damage(ops,&mut inner);
                regions.extend(inner[before..].iter().map(|r|r.intersect(*rect)).filter(|r|!r.is_empty()));
            }
            Op::Transformed { transform, ops } => {
                if let Some(inverse)=transform.inverse() {
                    let mut inner:Vec<_>=regions.iter().map(|r|inverse.bounds(*r)).collect();
                    let before=inner.len();
                    backdrop_damage(ops,&mut inner);
                    regions.extend(inner[before..].iter().map(|r|transform.bounds(*r)));
                }
            }
            _=>{}
        }
    }
}

fn damage_ops(old: &[Op], new: &[Op]) -> Option<Rect> {
    let mut regions = Vec::new();
    collect_damage(old, new, None, &mut regions);
    backdrop_damage(new, &mut regions);
    backdrop_damage(old, &mut regions);
    regions.into_iter().reduce(Rect::union)
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
    fn changes_far_apart_damage_only_themselves() {
        let old = list((0..40).map(|i| fill(i as f32 * 20.0)).collect());
        let mut ops: Vec<Op> = (0..40).map(|i| fill(i as f32 * 20.0)).collect();
        ops.insert(2, fill(2000.0));
        ops[39] = fill(3000.0);
        let new = list(ops);
        let regions = new.damage_regions(&old);
        let area: f32 = regions.iter().map(|r| r.width() * r.height()).sum();
        assert!(area < 4.0 * 12.0 * 12.0, "only the inserted and the changed op: {regions:?}");
    }

    #[test]
    fn ops_translate_and_scale_with_what_they_hold() {
        let mut op = Op::RoundedClipped {
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            radius: 4.0,
            ops: Arc::from([Op::Line {
                from: Point::new(10.0, 10.0),
                to: Point::new(30.0, 10.0),
                width: 2.0,
                color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            }]),
        };
        op.translate(5.0, -5.0);
        op.scale(2.0);
        let Op::RoundedClipped { rect, radius, ops } = op else { unreachable!() };
        assert_eq!((rect, radius), (Rect::new(30.0, 10.0, 40.0, 40.0), 8.0));
        let Op::Line { from, to, width, .. } = &ops[0] else { unreachable!() };
        assert_eq!((*from, *to, *width), (Point::new(30.0, 10.0), Point::new(70.0, 10.0), 4.0));
    }

    #[test]
    fn an_inserted_op_damages_only_itself() {
        let old = list((0..20).map(|i| fill(i as f32 * 20.0)).collect());
        let mut ops: Vec<Op> = (0..20).map(|i| fill(i as f32 * 20.0)).collect();
        ops.insert(1, fill(500.0));
        let new = list(ops);
        // The fill's own bounds, with its antialiasing pixel.
        let only = fill(500.0).bounds();
        assert_eq!(new.damage(&old), Some(only));
        assert_eq!(new.damage_regions(&old), vec![only]);
    }

    #[test]
    fn an_equal_op_that_leaves_a_clip_is_still_damaged() {
        // B is the same op in both lists, but only the old one draws inside the clip: the
        // part of it outside the clip appears, so it must be repainted.
        let clip = Op::Clip { rect: Rect::new(0.0, 0.0, 15.0, 10.0) };
        let (a, b) = (fill(0.0), fill(10.0));
        let old = list(vec![clip.clone(), a.clone(), b.clone(), Op::Unclip]);
        let new = list(vec![clip, a, Op::Unclip, b.clone()]);
        for (after, before) in [(&new, &old), (&old, &new)] {
            let damage = after.damage(before).expect("damage");
            assert_eq!(damage.union(b.bounds()), damage, "B's bounds repaint");
            let regions = after.damage_regions(before);
            assert!(regions.iter().any(|r| r.union(b.bounds()) == *r), "{regions:?}");
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
