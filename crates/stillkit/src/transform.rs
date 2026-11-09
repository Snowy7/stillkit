//! Draw-time affine coordinates shared by renderers and input.
use crate::{Point, Rect};
/// Maps a point to `(a*x + c*y + tx, b*x + d*y + ty)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform(
    /// Coefficients in the order `a, b, c, d, tx, ty`.
    pub [f32; 6],
);
impl Transform {
    /// The unchanged coordinate system.
    pub const IDENTITY: Self = Self([1., 0., 0., 1., 0., 0.]);
    /// Clockwise rotation and scale about `origin`, then translation.
    pub fn new(origin: Point, angle: f32, scale: (f32, f32), offset: (f32, f32)) -> Self {
        let (s, c) = angle.sin_cos();
        let (a, b, c, d) = (c * scale.0, s * scale.0, -s * scale.1, c * scale.1);
        Self([
            a,
            b,
            c,
            d,
            origin.x - a * origin.x - c * origin.y + offset.0,
            origin.y - b * origin.x - d * origin.y + offset.1,
        ])
    }
    /// Transforms a point.
    pub fn point(self, p: Point) -> Point {
        let [a, b, c, d, x, y] = self.0;
        Point::new(a * p.x + c * p.y + x, b * p.x + d * p.y + y)
    }
    /// Applies `inner` first, then this transform.
    pub fn compose(self, inner: Self) -> Self {
        let [a, b, c, d, x, y] = self.0;
        let [e, f, g, h, u, v] = inner.0;
        Self([
            a * e + c * f,
            b * e + d * f,
            a * g + c * h,
            b * g + d * h,
            a * u + c * v + x,
            b * u + d * v + y,
        ])
    }
    /// Inverts a transform; a collapsed transform has no inverse.
    pub fn inverse(self) -> Option<Self> {
        let [a, b, c, d, x, y] = self.0;
        let det = a * d - b * c;
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        Some(Self([
            d / det,
            -b / det,
            -c / det,
            a / det,
            (c * y - d * x) / det,
            (b * x - a * y) / det,
        ]))
    }
    /// Bounds enclosing a transformed rectangle.
    pub fn bounds(self, r: Rect) -> Rect {
        let p = [
            Point::new(r.left, r.top),
            Point::new(r.right, r.top),
            Point::new(r.left, r.bottom),
            Point::new(r.right, r.bottom),
        ]
        .map(|p| self.point(p));
        Rect {
            left: p.iter().map(|p| p.x).fold(f32::INFINITY, f32::min),
            top: p.iter().map(|p| p.y).fold(f32::INFINITY, f32::min),
            right: p.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max),
            bottom: p.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max),
        }
    }
}
