//! Every `Op` drawn with CoreGraphics and CoreText, in a top-left user space (the context's
//! CTM flips once, at creation). Strokes follow the Windows renderer (flat caps), icons the
//! Lucide outlines with round caps and joins, as Linux draws them.
use super::Text;
use crate::{DisplayList, Image, Op, PixelRect, Rect, Rgba};
use objc2_core_foundation::{CFData, CFRetained, CGAffineTransform, CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGColor, CGColorRenderingIntent, CGContext, CGDataProvider, CGImage, CGInterpolationQuality, CGLineCap,
    CGLineJoin, CGMutablePath, CGPath,
};
use std::{collections::HashMap, f64::consts::FRAC_PI_2, sync::Arc};

/// CGImages of the images a canvas drew last frame, by the `Arc`'s address (kept alive with
/// it); the rest are dropped after each draw.
#[derive(Default)]
pub(super) struct Images(HashMap<usize, (Arc<Image>, CFRetained<CGImage>, bool)>);

impl Images {
    fn get(&mut self, image: &Arc<Image>) -> Option<CFRetained<CGImage>> {
        let key = Arc::as_ptr(image) as usize;
        if let Some((_, cg, used)) = self.0.get_mut(&key) {
            *used = true;
            return Some(cg.clone());
        }
        let cg = cg_image(image)?;
        self.0.insert(key, (image.clone(), cg.clone(), true));
        Some(cg)
    }

    /// Forgets what this frame didn't draw; marks the rest unused for the next.
    fn sweep(&mut self) {
        self.0.retain(|_, (_, _, used)| std::mem::take(used));
    }
}

/// A CGImage over a copy of `image`'s premultiplied BGRA pixels.
pub(super) fn cg_image(image: &Image) -> Option<CFRetained<CGImage>> {
    let data = CFData::from_bytes(&image.pixels);
    let provider = CGDataProvider::with_cf_data(Some(&data))?;
    // SAFETY: the provider holds `width * height * 4` bytes in the layout described; no decode.
    unsafe {
        CGImage::new(
            image.width as usize,
            image.height as usize,
            8,
            32,
            image.width as usize * 4,
            Some(&super::render::srgb()),
            objc2_core_graphics::CGBitmapInfo(super::render::bgra()),
            Some(&provider),
            std::ptr::null(),
            true,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
}

/// Sets up a new bitmap context: cleared (CoreGraphics doesn't promise zeroed pixels), a
/// top-left user space, and grayscale text without font smoothing (a transparent bitmap has no
/// background to smooth against).
pub(super) fn prepare(ctx: &CGContext, (width, height): (u32, u32)) {
    let c = Some(ctx);
    CGContext::clear_rect(c, CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(width as f64, height as f64)));
    CGContext::translate_ctm(c, 0.0, height as f64);
    CGContext::scale_ctm(c, 1.0, -1.0);
    CGContext::set_should_smooth_fonts(c, false);
    CGContext::set_allows_font_subpixel_positioning(c, true);
    CGContext::set_should_subpixel_position_fonts(c, true);
    CGContext::set_interpolation_quality(c, CGInterpolationQuality::Default);
}

fn cg(r: Rect) -> CGRect {
    CGRect::new(CGPoint::new(r.left as f64, r.top as f64), CGSize::new(r.width().max(0.0) as f64, r.height().max(0.0) as f64))
}

fn color(c: Rgba) -> CFRetained<CGColor> {
    CGColor::new_srgb(c.r as f64, c.g as f64, c.b as f64, c.a as f64)
}

/// A rectangle with corners of `radius` (clamped to fit, as CoreGraphics requires).
fn rounded(r: Rect, radius: f32) -> CFRetained<CGPath> {
    let radius = (radius as f64).clamp(0.0, (r.width().min(r.height()) as f64 / 2.0).max(0.0));
    // SAFETY: a null transform is the identity; the radius fits the rectangle.
    unsafe { CGPath::with_rounded_rect(cg(r), radius, radius, std::ptr::null()) }
}

fn fill(ctx: &CGContext, path: &CGPath, c: Rgba) {
    CGContext::set_fill_color_with_color(Some(ctx), Some(&color(c)));
    CGContext::add_path(Some(ctx), Some(path));
    CGContext::fill_path(Some(ctx));
}

fn stroke(ctx: &CGContext, path: &CGPath, width: f32, c: Rgba, round: bool) {
    let x = Some(ctx);
    CGContext::set_stroke_color_with_color(x, Some(&color(c)));
    CGContext::set_line_width(x, width as f64);
    CGContext::set_line_cap(x, if round { CGLineCap::Round } else { CGLineCap::Butt });
    CGContext::set_line_join(x, if round { CGLineJoin::Round } else { CGLineJoin::Miter });
    CGContext::add_path(x, Some(path));
    CGContext::stroke_path(x);
}

fn polyline(points: impl IntoIterator<Item = (f64, f64)>, close: bool) -> CFRetained<CGMutablePath> {
    let path = CGMutablePath::new();
    for (i, (x, y)) in points.into_iter().enumerate() {
        // SAFETY: a live path; null transforms are the identity.
        unsafe {
            if i == 0 {
                CGMutablePath::move_to_point(Some(&path), std::ptr::null(), x, y);
            } else {
                CGMutablePath::add_line_to_point(Some(&path), std::ptr::null(), x, y);
            }
        }
    }
    if close {
        CGMutablePath::close_subpath(Some(&path));
    }
    path
}

/// Clears `area` and draws the operations of `list` that touch it, clipped to it.
pub(super) fn region(ctx: &CGContext, text: &mut Text, images: &mut Images, list: &DisplayList, area: PixelRect) {
    let c = Some(ctx);
    let clip = area.to_rect();
    CGContext::save_g_state(c);
    CGContext::clip_to_rect(c, cg(clip));
    CGContext::clear_rect(c, cg(clip));
    ops(ctx, text, images, &list.ops, clip);
    CGContext::restore_g_state(c);
    images.sweep();
}

/// Marker clips are scoped to this slice (a retained group's included), so an unmatched
/// `Unclip` never pops a caller's clip.
fn ops(ctx: &CGContext, text: &mut Text, images: &mut Images, ops_: &[Op], damage: Rect) {
    let mut pushed = 0;
    for op in ops_ {
        match op {
            Op::Clip { rect } => {
                CGContext::save_g_state(Some(ctx));
                CGContext::clip_to_rect(Some(ctx), cg(*rect));
                pushed += 1;
            }
            Op::Unclip if pushed > 0 => {
                CGContext::restore_g_state(Some(ctx));
                pushed -= 1;
            }
            Op::Unclip => {}
            op if op.bounds().intersects(&damage) => self::op(ctx, text, images, op, damage),
            _ => {}
        }
    }
    for _ in 0..pushed {
        CGContext::restore_g_state(Some(ctx));
    }
}

fn op(ctx: &CGContext, text: &mut Text, images: &mut Images, op: &Op, damage: Rect) {
    let c = Some(ctx);
    match op {
        Op::Clipped { rect, ops: inner } => {
            CGContext::save_g_state(c);
            CGContext::clip_to_rect(c, cg(*rect));
            ops(ctx, text, images, inner, rect.intersect(damage));
            CGContext::restore_g_state(c);
        }
        Op::RoundedClipped { rect, radius, ops: inner } => {
            CGContext::save_g_state(c);
            CGContext::add_path(c, Some(&rounded(*rect, *radius)));
            CGContext::clip(c);
            ops(ctx, text, images, inner, rect.intersect(damage));
            CGContext::restore_g_state(c);
        }
        Op::Gradient { rect, radius, width, gradient } => {
            use objc2_core_graphics::{CGColorSpace, CGGradient, CGGradientDrawingOptions};
            let components: Vec<_> = gradient.stops.iter().flat_map(|s| [s.color.r as f64, s.color.g as f64, s.color.b as f64, s.color.a as f64]).collect();
            let locations: Vec<_> = gradient.stops.iter().map(|s| s.offset as f64).collect();
            let space = CGColorSpace::new_device_rgb();
            // SAFETY: four components and one location for every stop, kept alive for this call.
            let g = unsafe { CGGradient::with_color_components(space.as_deref(), components.as_ptr(), locations.as_ptr(), locations.len()) };
            if let Some(g) = g {
                let c = Some(ctx);
                CGContext::save_g_state(c);
                CGContext::add_path(c, Some(&rounded(*rect, *radius)));
                if *width > 0.0 {
                    CGContext::set_line_width(c, *width as f64);
                    CGContext::set_line_cap(c, CGLineCap::Butt);
                    CGContext::set_line_join(c, CGLineJoin::Miter);
                    CGContext::replace_path_with_stroked_path(c);
                }
                CGContext::clip(c);
                let options = CGGradientDrawingOptions::DrawsBeforeStartLocation | CGGradientDrawingOptions::DrawsAfterEndLocation;
                if gradient.angle.is_some() {
                    let (a,b) = gradient.line(*rect);
                    CGContext::draw_linear_gradient(c, Some(&g), CGPoint::new(a.x as f64, a.y as f64), CGPoint::new(b.x as f64, b.y as f64), options);
                } else {
                    let at = rect.center();
                    let center = CGPoint::new(at.x as f64, at.y as f64);
                    CGContext::draw_radial_gradient(c, Some(&g), center, 0.0, center, ((rect.width() * rect.width() + rect.height() * rect.height()).sqrt() * 0.5).max(0.001) as f64, options);
                }
                CGContext::restore_g_state(c);
            }
        }
        Op::Fill { rect, radius, color } => fill(ctx, &rounded(*rect, *radius), *color),
        Op::Hole { rect } => CGContext::clear_rect(c, cg(*rect)),
        Op::Stroke { rect, radius, width, color } => stroke(ctx, &rounded(*rect, *radius), *width, *color, false),
        Op::Continuous { rect, radius, smoothing, width, color } => {
            let points = crate::geom::continuous::outline(*rect, *radius, *smoothing);
            let path = polyline(points.iter().map(|p| (p.x as f64, p.y as f64)), true);
            if *width > 0.0 {
                stroke(ctx, &path, *width, *color, false);
            } else {
                fill(ctx, &path, *color);
            }
        }
        Op::Line { from, to, width, color } => {
            let path = polyline([(from.x as f64, from.y as f64), (to.x as f64, to.y as f64)], false);
            stroke(ctx, &path, *width, *color, false);
        }
        Op::Arc { center, radius, start, sweep, width, color } => {
            let path = CGMutablePath::new();
            let (x, y, r) = (center.x as f64, center.y as f64, *radius as f64);
            if *sweep >= std::f32::consts::TAU - 0.0001 {
                // SAFETY: a live path; a null transform is the identity.
                unsafe { CGMutablePath::add_ellipse_in_rect(Some(&path), std::ptr::null(), CGRect::new(CGPoint::new(x - r, y - r), CGSize::new(r * 2.0, r * 2.0))) };
            } else if *sweep > 0.0 {
                // From `start` past twelve o'clock, clockwise on screen (increasing angles in
                // y-down space, from three o'clock).
                let from = -FRAC_PI_2 + *start as f64;
                // SAFETY: as above; the arc starts the path at its first point.
                unsafe {
                    CGMutablePath::move_to_point(Some(&path), std::ptr::null(), x + r * from.cos(), y + r * from.sin());
                    CGMutablePath::add_relative_arc(Some(&path), std::ptr::null(), x, y, r, from, *sweep as f64);
                }
            } else {
                return;
            }
            stroke(ctx, &path, *width, *color, false);
        }
        Op::Icon { rect, icon, weight, color } => {
            let path = icon_path(*icon, *rect);
            if icon.filled() {
                fill(ctx, &path, *color);
            }
            stroke(ctx, &path, *weight, *color, true);
        }
        Op::Text { rect, text: label, style, color } | Op::Emphasis { rect, text: label, style, color, .. } => {
            if label.chars().count() == 1
                && let Some(icon) = label.chars().next().and_then(crate::icons::from_system_glyph)
            {
                let icon = Op::Icon { rect: Rect::new(rect.left, rect.top, style.size, style.size), icon, weight: (style.size / 12.).max(1.), color: *color };
                return self::op(ctx, text, images, &icon, damage);
            }
            let italic = matches!(op, Op::Emphasis { italic: true, .. });
            let layout = text.shape(label, style, italic);
            CGContext::set_fill_color_with_color(c, Some(&self::color(*color)));
            CGContext::set_text_matrix(c, CGAffineTransform { a: 1.0, b: 0.0, c: 0.0, d: -1.0, tx: 0.0, ty: 0.0 });
            for (i, (line, _)) in layout.lines.iter().enumerate() {
                let baseline = rect.top as f64 + layout.ascent + i as f64 * layout.line_height;
                CGContext::set_text_position(c, rect.left as CGFloat, baseline);
                // SAFETY: a live line drawn into a live context.
                unsafe { line.draw(ctx) };
            }
            if matches!(op, Op::Emphasis { strike: true, .. }) {
                let y = (rect.top + style.size * 0.65) as f64;
                stroke(ctx, &polyline([(rect.left as f64, y), (rect.right as f64, y)], false), 1.0, *color, false);
            }
        }
        Op::Image { rect, image, opacity, source } | Op::RoundedImage { rect, image, opacity, source, .. } => {
            let Some(cg_image) = images.get(image) else { return };
            let src = source.unwrap_or(Rect::new(0.0, 0.0, image.width as f32, image.height as f32));
            let (sx, sy) = ((rect.width() / src.width()) as f64, (rect.height() / src.height()) as f64);
            // The whole image placed so `src` lands on `rect`, clipped to `rect` (rounded).
            let (dx, dy) = (rect.left as f64 - src.left as f64 * sx, rect.top as f64 - src.top as f64 * sy);
            let (dw, dh) = (image.width as f64 * sx, image.height as f64 * sy);
            CGContext::save_g_state(c);
            let radius = if let Op::RoundedImage { radius, .. } = op { *radius } else { 0.0 };
            CGContext::add_path(c, Some(&rounded(*rect, radius)));
            CGContext::clip(c);
            CGContext::set_alpha(c, *opacity as f64);
            // CoreGraphics draws images bottom-up: flip around the destination.
            CGContext::translate_ctm(c, dx, dy + dh);
            CGContext::scale_ctm(c, 1.0, -1.0);
            CGContext::draw_image(c, CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(dw, dh)), Some(&cg_image));
            CGContext::restore_g_state(c);
        }
        Op::Shadow { rect, radius, spread, drop, color } => {
            // Rings two pixels apart, fading outward, as the other renderers draw it.
            let steps = (spread / 2.0).floor() as i32;
            for i in 0..steps {
                let grow = (i * 2) as f32;
                let fade = 1.0 - i as f32 / steps as f32;
                fill(ctx, &rounded(rect.inflate(grow).offset(0.0, *drop), radius + grow), color.with_alpha(color.a * fade));
            }
        }
        Op::SoftShadow { rect, radius, blur, offset, spread, color } => {
            soft_shadow(ctx, *rect, *radius, *blur, *offset, *spread, *color)
        }
        Op::TextField { rect, field, .. } => self::op(ctx, text, images, &field.op(*rect), damage),
        Op::Clip { .. } | Op::Unclip => {}
    }
}

/// [`Op::SoftShadow`]: a Gaussian blur of `blur` (2σ) of the rounded shape grown by `spread`
/// and moved by `offset`, in `color`, painted under the shape too (as the other backends do;
/// a translucent panel shows its shadow through it). CoreGraphics casts it exactly: the shape
/// is filled far outside the bitmap, and its shadow, offset back, is all that lands.
pub(super) fn soft_shadow(ctx: &CGContext, rect: Rect, radius: f32, blur: f32, offset: (f32, f32), spread: f32, c: Rgba) {
    let x = Some(ctx);
    let shape = rect.inflate(spread);
    // Far enough that neither the shape nor its blur's reach touches the bitmap.
    let away = (shape.right.max(0.0) + blur * 3.0 + offset.0.abs() + 1.0).ceil() as f64 + 16384.0;
    CGContext::save_g_state(x);
    // Shadow offsets are in device space, unflipped: y goes up there.
    CGContext::set_shadow_with_color(x, CGSize::new(offset.0 as f64 + away, -(offset.1 as f64)), blur as f64, Some(&color(c)));
    let moved = shape.offset(-away as f32, 0.0);
    fill(ctx, &rounded(moved, radius + spread), Rgba { a: 1.0, ..c });
    CGContext::restore_g_state(x);
}

/// `icon`'s outline (on its 24-unit grid) scaled into `rect`.
fn icon_path(icon: crate::icons::Icon, rect: Rect) -> CFRetained<CGMutablePath> {
    use crate::icons::Seg;
    let (sx, sy) = (rect.width() as f64 / 24.0, rect.height() as f64 / 24.0);
    let pt = |p: crate::Point| (rect.left as f64 + p.x as f64 * sx, rect.top as f64 + p.y as f64 * sy);
    let path = CGMutablePath::new();
    let p = Some(&*path);
    let none = std::ptr::null();
    let mut at = (0.0, 0.0);
    for seg in icon.segments() {
        // SAFETY: a live path; null transforms are the identity.
        unsafe {
            match seg {
                Seg::Move(to) => {
                    at = pt(to);
                    CGMutablePath::move_to_point(p, none, at.0, at.1);
                }
                Seg::Line(to) => {
                    at = pt(to);
                    CGMutablePath::add_line_to_point(p, none, at.0, at.1);
                }
                Seg::Quad(a, to) => {
                    let a = pt(a);
                    at = pt(to);
                    CGMutablePath::add_quad_curve_to_point(p, none, a.0, a.1, at.0, at.1);
                }
                Seg::Cubic(a, b, to) => {
                    let (a, b) = (pt(a), pt(b));
                    at = pt(to);
                    CGMutablePath::add_curve_to_point(p, none, a.0, a.1, b.0, b.1, at.0, at.1);
                }
                Seg::Close => CGMutablePath::close_subpath(p),
                Seg::Arc { to, rx, ry, rotation, large, sweep } => {
                    let to = pt(to);
                    svg_arc(&path, at, to, (rx as f64 * sx, ry as f64 * sy), rotation as f64, large, sweep);
                    at = to;
                }
            }
        }
    }
    path
}

/// An SVG elliptical arc from `from` to `to` (SVG 1.1, F.6.5: endpoint to centre form), as a
/// unit-circle arc under the ellipse's transform. A degenerate one is a line.
fn svg_arc(path: &CGMutablePath, from: (f64, f64), to: (f64, f64), (rx, ry): (f64, f64), rotation: f64, large: bool, sweep: bool) {
    let p = Some(path);
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if from == to {
        return;
    }
    if rx == 0.0 || ry == 0.0 {
        // SAFETY: a live path; a null transform is the identity.
        unsafe { CGMutablePath::add_line_to_point(p, std::ptr::null(), to.0, to.1) };
        return;
    }
    let (sin, cos) = rotation.to_radians().sin_cos();
    let (dx, dy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
    let (x1, y1) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let lambda = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let k = (num / den).max(0.0).sqrt() * if large == sweep { -1.0 } else { 1.0 };
    let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let (cx, cy) = (cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0, sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0);
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let (ux, uy) = ((x1 - cx1) / rx, (y1 - cy1) / ry);
    let (vx, vy) = ((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let start = angle(1.0, 0.0, ux, uy);
    let mut delta = angle(ux, uy, vx, vy);
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let m = CGAffineTransform { a: rx * cos, b: rx * sin, c: -ry * sin, d: ry * cos, tx: cx, ty: cy };
    // SAFETY: a live path and a valid transform.
    unsafe { CGMutablePath::add_relative_arc(p, &m, 0.0, 0.0, 1.0, start, delta) };
}
