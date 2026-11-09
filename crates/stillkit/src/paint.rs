//! Gradient geometry shared by the native renderers.
use crate::{Point, Rect, Rgba};
use std::sync::Arc;

/// A colour at a position along a gradient, clamped to the unit interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    /// Position from zero to one.
    pub offset: f32,
    /// Unpremultiplied sRGB colour.
    pub color: Rgba,
}

/// A gradient in the painted rectangle's coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Stops in increasing order; empty gradients are transparent.
    pub stops: Arc<[GradientStop]>,
    /// Clockwise radians from the right; `None` is a centred radial gradient.
    pub angle: Option<f32>,
}

impl Gradient {
    /// Sorts stops and clamps their positions before handing them to a native brush.
    pub fn new(stops: &[GradientStop], angle: Option<f32>) -> Self {
        let mut stops: Vec<_> = stops
            .iter()
            .copied()
            .map(|mut s| {
                s.offset = if s.offset.is_finite() {
                    s.offset.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                s
            })
            .collect();
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        if stops.is_empty() {
            stops.push(GradientStop {
                offset: 0.0,
                color: Rgba::TRANSPARENT,
            });
        }
        if stops.len() == 1 {
            stops.push(GradientStop {
                offset: 1.0,
                ..stops[0]
            });
        }
        Self {
            stops: stops.into(),
            angle: angle.map(|a| if a.is_finite() { a } else { 0.0 }),
        }
    }

    /// The ends of a linear gradient, covering the whole rectangle at any angle.
    pub fn line(&self, rect: Rect) -> (Point, Point) {
        let (sin, cos) = self.angle.unwrap_or(0.0).sin_cos();
        let reach = (rect.width() * cos.abs() + rect.height() * sin.abs()) * 0.5;
        let c = rect.center();
        (
            Point::new(c.x - cos * reach, c.y - sin * reach),
            Point::new(c.x + cos * reach, c.y + sin * reach),
        )
    }

    pub(crate) fn fade(&mut self, by: f32) {
        for stop in Arc::make_mut(&mut self.stops) {
            stop.color.a *= by;
        }
    }
}
