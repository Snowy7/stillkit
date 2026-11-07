//! Draw-time rotation ([`super::El::rotate`], [`super::El::spin`]): an element's painted ops
//! turned about its centre, without a transform in the renderers. What a turn maps onto an
//! op of the same kind turns exactly: arcs (their start angle), lines, circles, every box at
//! quarter turns, and icons, which are drawn as their stroked outline while turned (lines
//! with round joins and caps). Text, images, filled icons and boxes at other angles keep
//! upright, about their turned centre. Clips at other angles grow to the turned box.

use std::f32::consts::{FRAC_PI_2, TAU};

use crate::icons::Seg;
use crate::{Op, Point, Rect};

/// A turn of `angle` radians clockwise (on screen) about `c`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Turn {
    c: Point,
    angle: f32,
    sin: f32,
    cos: f32,
    /// Quarter turns, when it is a whole number of them.
    quarters: Option<u8>,
}

impl Turn {
    pub fn new(c: Point, angle: f32) -> Self {
        let k = angle / FRAC_PI_2;
        let quarters = ((k - k.round()).abs() < 1e-4).then(|| (k.round() as i64).rem_euclid(4) as u8);
        let (sin, cos) = match quarters {
            Some(0) => (0.0, 1.0),
            Some(1) => (1.0, 0.0),
            Some(2) => (0.0, -1.0),
            Some(3) => (-1.0, 0.0),
            _ => angle.sin_cos(),
        };
        Self { c, angle, sin, cos, quarters }
    }

    /// Whether it turns nothing.
    pub fn none(&self) -> bool {
        self.quarters == Some(0)
    }

    pub fn point(&self, p: Point) -> Point {
        let (dx, dy) = (p.x - self.c.x, p.y - self.c.y);
        Point::new(self.c.x + dx * self.cos - dy * self.sin, self.c.y + dx * self.sin + dy * self.cos)
    }

    /// The box around `r` turned.
    pub fn bounds(&self, r: Rect) -> Rect {
        let corners = [(r.left, r.top), (r.right, r.top), (r.left, r.bottom), (r.right, r.bottom)]
            .map(|(x, y)| self.point(Point::new(x, y)));
        let (xs, ys) = (corners.map(|p| p.x), corners.map(|p| p.y));
        let (min, max) = (|v: [f32; 4]| v.into_iter().fold(f32::MAX, f32::min), |v: [f32; 4]| v.into_iter().fold(f32::MIN, f32::max));
        Rect { left: min(xs), top: min(ys), right: max(xs), bottom: max(ys) }
    }

    /// `r` about its turned centre: turned exactly at quarter turns (and for a circle, where
    /// `round` says it is one), else upright.
    fn rect(&self, r: Rect, round: bool) -> Rect {
        if self.quarters.is_some() {
            return self.bounds(r);
        }
        let c = r.center();
        let to = self.point(c);
        let (dx, dy) = (to.x - c.x, to.y - c.y);
        // Upright content stays on whole pixels; a circle needn't.
        let (dx, dy) = if round { (dx, dy) } else { (dx.round(), dy.round()) };
        r.offset(dx, dy)
    }

    fn vector(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (x * self.cos - y * self.sin, x * self.sin + y * self.cos)
    }

    /// `ops` turned.
    pub fn ops(&self, ops: Vec<Op>) -> Vec<Op> {
        let mut out = Vec::with_capacity(ops.len());
        for op in ops {
            self.op(op, &mut out);
        }
        out
    }

    fn op(&self, op: Op, out: &mut Vec<Op>) {
        let circle = |r: Rect, radius: f32| (r.width() - r.height()).abs() < 0.5 && radius >= r.width() / 2.0 - 0.5;
        out.push(match op {
            Op::Arc { center, radius, start, sweep, width, color } => Op::Arc {
                center: self.point(center),
                radius,
                start: (start + self.angle).rem_euclid(TAU),
                sweep,
                width,
                color,
            },
            Op::Line { from, to, width, color } => Op::Line { from: self.point(from), to: self.point(to), width, color },
            Op::Fill { rect, radius, color } => Op::Fill { rect: self.rect(rect, circle(rect, radius)), radius, color },
            Op::Stroke { rect, radius, width, color } => {
                Op::Stroke { rect: self.rect(rect, circle(rect, radius)), radius, width, color }
            }
            Op::Continuous { rect, radius, smoothing, width, color } => {
                Op::Continuous { rect: self.rect(rect, circle(rect, radius)), radius, smoothing, width, color }
            }
            Op::SoftShadow { rect, radius, blur, offset, spread, color } => Op::SoftShadow {
                rect: self.rect(rect, circle(rect, radius)),
                radius,
                blur,
                offset: self.vector(offset),
                spread,
                color,
            },
            Op::Shadow { rect, radius, spread, drop, color } => {
                Op::Shadow { rect: self.rect(rect, false), radius, spread, drop, color }
            }
            Op::Icon { rect, icon, weight, color } if !icon.filled() && self.quarters != Some(0) => {
                let segs = icon.segments();
                if segs.is_empty() {
                    Op::Icon { rect: self.rect(rect, false), icon, weight, color }
                } else {
                    return self.outline(&segs, rect, weight, color, out);
                }
            }
            Op::Icon { rect, icon, weight, color } => Op::Icon { rect: self.rect(rect, false), icon, weight, color },
            Op::Text { rect, text, style, color } => Op::Text { rect: self.rect(rect, false), text, style, color },
            Op::Emphasis { rect, text, style, color, italic, strike } => {
                Op::Emphasis { rect: self.rect(rect, false), text, style, color, italic, strike }
            }
            Op::Image { rect, image, opacity, source } => Op::Image { rect: self.rect(rect, false), image, opacity, source },
            Op::RoundedImage { rect, image, opacity, source, radius } => {
                Op::RoundedImage { rect: self.rect(rect, false), image, opacity, source, radius }
            }
            Op::TextField { rect, field, revision } => Op::TextField { rect: self.rect(rect, false), field, revision },
            Op::Clipped { rect, ops } => Op::Clipped { rect: self.bounds(rect), ops: self.ops(ops.to_vec()).into() },
            Op::RoundedClipped { rect, radius, ops } => {
                Op::RoundedClipped { rect: self.bounds(rect), radius, ops: self.ops(ops.to_vec()).into() }
            }
            Op::Clip { rect } => Op::Clip { rect: self.bounds(rect) },
            Op::Unclip => Op::Unclip,
        });
    }

    /// An icon's outline (on Lucide's 24-unit grid, filling `rect`) turned, stroked `weight`
    /// wide: a line per stretch (curves in short ones) and a disc at every joint and end,
    /// which are its round joins and caps.
    fn outline(&self, segs: &[Seg], rect: Rect, weight: f32, color: crate::Rgba, out: &mut Vec<Op>) {
        let (sx, sy) = (rect.width() / 24.0, rect.height() / 24.0);
        let at = |p: Point| self.point(Point::new(rect.left + p.x * sx, rect.top + p.y * sy));
        let mut lines: Vec<(Point, Point)> = Vec::new();
        let (mut pen, mut first) = (Point::new(0.0, 0.0), Point::new(0.0, 0.0));
        let to = |lines: &mut Vec<(Point, Point)>, pen: &mut Point, p: Point| {
            lines.push((*pen, p));
            *pen = p;
        };
        for seg in segs {
            match *seg {
                Seg::Move(p) => (pen, first) = (p, p),
                Seg::Line(p) => to(&mut lines, &mut pen, p),
                Seg::Close => to(&mut lines, &mut pen, first),
                Seg::Cubic(a, b, p) => {
                    let p0 = pen;
                    for i in 1..=STEPS {
                        let t = i as f32 / STEPS as f32;
                        let u = 1.0 - t;
                        let q = |c0: f32, c1: f32, c2: f32, c3: f32| u * u * u * c0 + 3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t * c3;
                        to(&mut lines, &mut pen, Point::new(q(p0.x, a.x, b.x, p.x), q(p0.y, a.y, b.y, p.y)));
                    }
                }
                Seg::Quad(a, p) => {
                    let p0 = pen;
                    for i in 1..=STEPS {
                        let t = i as f32 / STEPS as f32;
                        let u = 1.0 - t;
                        let q = |c0: f32, c1: f32, c2: f32| u * u * c0 + 2.0 * u * t * c1 + t * t * c2;
                        to(&mut lines, &mut pen, Point::new(q(p0.x, a.x, p.x), q(p0.y, a.y, p.y)));
                    }
                }
                Seg::Arc { to: p, rx, ry, rotation, large, sweep } => {
                    for q in arc_points(pen, p, rx, ry, rotation, large, sweep) {
                        to(&mut lines, &mut pen, q);
                    }
                }
            }
        }
        let disc = |p: Point| Op::Fill {
            rect: Rect::new(p.x - weight / 2.0, p.y - weight / 2.0, weight, weight),
            radius: weight / 2.0,
            color,
        };
        for &(a, b) in &lines {
            out.push(Op::Line { from: at(a), to: at(b), width: weight, color });
        }
        let mut joints: Vec<Point> = lines.iter().flat_map(|&(a, b)| [a, b]).collect();
        joints.dedup_by(|a, b| (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3);
        out.extend(joints.into_iter().map(|p| disc(at(p))));
    }
}

/// Short lines per curve when an icon is drawn as its outline.
const STEPS: usize = 8;

/// Points along SVG's elliptical arc from `from` to `to` (after `from`), by the endpoint to
/// centre conversion of the SVG spec (appendix B.2.4).
fn arc_points(from: Point, to: Point, rx: f32, ry: f32, rotation: f32, large: bool, sweep: bool) -> Vec<Point> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || (from.x == to.x && from.y == to.y) {
        return vec![to];
    }
    let (sin, cos) = rotation.to_radians().sin_cos();
    let (hx, hy) = ((from.x - to.x) / 2.0, (from.y - to.y) / 2.0);
    let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
    let grow = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if grow > 1.0 {
        (rx, ry) = (rx * grow.sqrt(), ry * grow.sqrt());
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let k = if den > 0.0 { (num / den).sqrt() } else { 0.0 } * if large == sweep { -1.0 } else { 1.0 };
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let (cx, cy) = (cos * cx1 - sin * cy1 + (from.x + to.x) / 2.0, sin * cx1 + cos * cy1 + (from.y + to.y) / 2.0);
    let angle = |ux: f32, uy: f32| uy.atan2(ux);
    let start = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry) - start;
    if sweep && delta < 0.0 {
        delta += TAU;
    } else if !sweep && delta > 0.0 {
        delta -= TAU;
    }
    let n = ((delta.abs() / (TAU / 24.0)).ceil() as usize).max(1);
    (1..=n)
        .map(|i| {
            let a = start + delta * i as f32 / n as f32;
            let (x, y) = (rx * a.cos(), ry * a.sin());
            Point::new(cos * x - sin * y + cx, sin * x + cos * y + cy)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgba;

    #[test]
    fn a_quarter_turn_maps_arcs_lines_and_boxes_exactly() {
        let turn = Turn::new(Point::new(50.0, 50.0), FRAC_PI_2);
        let color = Rgba::HIT;
        let ops = turn.ops(vec![
            Op::Arc { center: Point::new(50.0, 50.0), radius: 10.0, start: 0.0, sweep: 1.0, width: 2.0, color },
            Op::Line { from: Point::new(50.0, 40.0), to: Point::new(60.0, 40.0), width: 1.0, color },
            Op::Fill { rect: Rect::new(40.0, 45.0, 20.0, 10.0), radius: 2.0, color },
        ]);
        let Op::Arc { start, .. } = ops[0] else { panic!() };
        assert!((start - FRAC_PI_2).abs() < 1e-6, "twelve o'clock turns to three");
        assert_eq!(ops[1], Op::Line { from: Point::new(60.0, 50.0), to: Point::new(60.0, 60.0), width: 1.0, color });
        assert_eq!(ops[2], Op::Fill { rect: Rect::new(45.0, 40.0, 10.0, 20.0), radius: 2.0, color });
    }

    #[test]
    fn arc_points_follow_a_half_circle() {
        let pts = arc_points(Point::new(0.0, 0.0), Point::new(10.0, 0.0), 5.0, 5.0, 0.0, false, true);
        let last = *pts.last().unwrap();
        assert!((last.x - 10.0).abs() < 1e-3 && last.y.abs() < 1e-3);
        // Sweeping clockwise on screen from the left end goes up (y < 0) first.
        assert!(pts.iter().all(|p| ((p.x - 5.0).powi(2) + p.y.powi(2)).sqrt() - 5.0 < 1e-3));
        assert!(pts.iter().map(|p| p.y).fold(0.0, f32::min) < -4.99);
    }
}
