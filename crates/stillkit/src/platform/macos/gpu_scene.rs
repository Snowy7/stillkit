//! Display lists as Vello Hybrid scenes (GPU mode). Geometry follows the Linux scene builder;
//! text is CoreText's own shaping and glyph outlines (`CTFontCreatePathForGlyph`), so a GPU
//! surface lays out and draws exactly the glyphs the CPU renderer does, without a second font
//! stack.
use super::Text;
use crate::{Op, Rect, Rgba};
use anyhow::{Result, ensure};
use objc2_core_foundation::{CFRange, CFRetained, CGPoint};
use objc2_core_graphics::{CGPath, CGPathElement, CGPathElementType};
use objc2_core_text::{CTFont, CTLine, CTRun, kCTFontAttributeName};
use std::{cell::RefCell, collections::HashMap, ffi::c_void, ptr::NonNull, sync::Arc};
use vello_hybrid::{
    Pixmap, Resources, Scene,
    kurbo::{self, Affine, BezPath, Cap, Join, Shape, Stroke},
};

fn rect(r: Rect) -> kurbo::Rect {
    kurbo::Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)
}

fn rounded(r: Rect, radius: f32) -> BezPath {
    kurbo::RoundedRect::from_rect(rect(r), radius as f64).to_path(0.05)
}

fn paint(ctx: &mut Scene, c: Rgba) {
    ctx.set_paint(vello_hybrid::color::AlphaColor::<vello_hybrid::color::Srgb>::new([c.r, c.g, c.b, c.a]),
    );
}

/// Fills (`width == 0`) or strokes; round caps and joins only for icons, as on the CPU.
fn path(ctx: &mut Scene, p: &BezPath, width: f32, c: Rgba, round: bool) {
    paint(ctx, c);
    if width > 0. {
        let (cap, join) = if round { (Cap::Round, Join::Round) } else { (Cap::Butt, Join::Miter) };
        ctx.set_stroke(Stroke::new(width as f64).with_caps(cap).with_join(join));
        ctx.stroke_path(p);
    } else {
        ctx.fill_path(p);
    }
}

type Outlines = HashMap<(usize, u16), (CFRetained<CTFont>, BezPath)>;

thread_local! {
    /// Glyph outlines by font and glyph; the font is kept so its address stays its own.
    static GLYPHS: RefCell<Outlines> = RefCell::new(HashMap::new());
}

/// CGPathApply's callback: appends one element to the `BezPath` behind `info`.
unsafe extern "C-unwind" fn element(info: *mut c_void, e: NonNull<CGPathElement>) {
    // SAFETY: `info` is the `BezPath` `outline` passes; the element and its points are valid
    // for this call (one, one, two and three points by type).
    unsafe {
        let out = &mut *info.cast::<BezPath>();
        let e = e.as_ref();
        let p = |i: usize| {
            let p: CGPoint = *e.points.as_ptr().add(i);
            kurbo::Point::new(p.x, p.y)
        };
        match e.r#type {
            CGPathElementType::MoveToPoint => out.move_to(p(0)),
            CGPathElementType::AddLineToPoint => out.line_to(p(0)),
            CGPathElementType::AddQuadCurveToPoint => out.quad_to(p(0), p(1)),
            CGPathElementType::AddCurveToPoint => out.curve_to(p(0), p(1), p(2)),
            CGPathElementType::CloseSubpath => out.close_path(),
            _ => {}
        }
    }
}

/// A glyph's outline in font units at its size, y up.
fn outline(font: &CFRetained<CTFont>, glyph: u16) -> BezPath {
    let key = (CFRetained::as_ptr(font).as_ptr() as usize, glyph);
    if let Some(p) = GLYPHS.with_borrow(|g| g.get(&key).map(|(_, p)| p.clone())) {
        return p;
    }
    let mut out = BezPath::new();
    // SAFETY: a live font; a null matrix is the identity.
    if let Some(cg) = unsafe { font.path_for_glyph(glyph, std::ptr::null()) } {
        // SAFETY: `element` reads only the elements CGPathApply passes; `out` outlives the call.
        unsafe { CGPath::apply(Some(&cg), (&raw mut out).cast(), Some(element)) };
    }
    GLYPHS.with_borrow_mut(|g| {
        if g.len() >= 4096 {
            g.clear();
        }
        g.insert(key, (font.clone(), out.clone()));
    });
    out
}

/// Fills one shaped line's glyphs with its baseline at `origin`.
fn line(ctx: &mut Scene, line: &CTLine, origin: (f64, f64)) {
    // SAFETY: a live line; its runs are CTRuns.
    let runs = unsafe { line.glyph_runs() };
    // SAFETY: as above.
    let runs = unsafe { runs.cast_unchecked::<CTRun>() };
    for run in runs.iter() {
        // SAFETY: a live run: its attributes hold the font it was shaped with, and its glyph
        // and position buffers have `count` entries.
        unsafe {
            let attributes = run.attributes();
            let key: *const objc2_core_foundation::CFString = kCTFontAttributeName;
            let Some(font) = attributes.value(key.cast()).cast::<CTFont>().as_ref() else {
                continue;
            };
            let font = CFRetained::retain(NonNull::from(font));
            let count = run.glyph_count().max(0) as usize;
            let mut glyphs = vec![0u16; count];
            let mut positions = vec![CGPoint::new(0.0, 0.0); count];
            let all = CFRange { location: 0, length: 0,
            };
            if count > 0 {
                run.glyphs(all, NonNull::new_unchecked(glyphs.as_mut_ptr()));
                run.positions(all, NonNull::new_unchecked(positions.as_mut_ptr()));
            }
            for (g, at) in glyphs.into_iter().zip(positions) {
                let p = outline(&font, g);
                // Font space is y-up: flip it onto the baseline.
                let placed = Affine::translate((origin.0 + at.x, origin.1 - at.y)) * Affine::FLIP_Y * p;
                ctx.fill_path(&placed);
            }
        }
    }
}

pub(super) fn draw_ops(
    ctx: &mut Scene,
    res: &mut Resources,
    text: &mut Text,
    ops: &[Op],
    scene_transform: Affine,
    image_source: &mut impl FnMut(&mut Resources, Arc<Pixmap>) -> vello_hybrid::ImageSource,
) -> Result<()> {
    let prepared =
        crate::blur::prepare_native(text, ops, Rect::new(-32768.0, -32768.0, 65536.0, 65536.0))?;
    let ops = prepared.as_ref();
    let mut clips = 0;
    for op in ops {
        match op {
            Op::Blurred { sigma, ops } => {
                use vello_hybrid::filter_effects::{Filter, FilterFunction};
                ctx.push_filter_layer(Filter::from_function(FilterFunction::Blur {
                    radius: *sigma,
                }));
                draw_ops(ctx, res, text, ops, scene_transform, image_source)?;
                ctx.pop_layer();
            }
            Op::BackdropBlur { .. } => unreachable!("filters resolved before drawing"),
            Op::Transformed { transform, ops } => {
                let saved = scene_transform;
                ctx.set_transform(saved * Affine::new(transform.0.map(f64::from)));
                let result = draw_ops(ctx,res,text,ops,saved * Affine::new(transform.0.map(f64::from)),image_source,
                );
                ctx.set_transform(saved);
                result?;
            }
            Op::Hole { rect: r } => {
                ctx.set_blend_mode(vello_hybrid::peniko::BlendMode::new(vello_hybrid::peniko::Mix::Normal, vello_hybrid::peniko::Compose::Copy,
                ));
                paint(ctx, Rgba::TRANSPARENT);
                ctx.fill_path(&rounded(*r, 0.));
                // Nothing else changes the blend mode: back to source-over.
                ctx.set_blend_mode(Default::default());
            }
            Op::Clip { rect: r } => {
                ctx.push_clip_layer(&rounded(*r, 0.));
                clips += 1;
            }
            Op::Unclip => {
                if clips > 0 {
                    ctx.pop_layer();
                    clips -= 1;
                }
            }
            Op::Clipped { rect: r, ops } => {
                ctx.push_clip_layer(&rounded(*r, 0.));
                draw_ops(ctx, res, text, ops, scene_transform, image_source)?;
                ctx.pop_layer();
            }
            Op::RoundedClipped { rect: r, radius, ops,
            } => {
                ctx.push_clip_layer(&rounded(*r, *radius));
                draw_ops(ctx, res, text, ops, scene_transform, image_source)?;
                ctx.pop_layer();
            }
            Op::Gradient { rect: r, radius, width, gradient,
            } => {
                use vello_hybrid::peniko::{ColorStop, Gradient};
                let stops: Vec<_> = gradient.stops.iter().map(|s| ColorStop { offset: s.offset, color: vello_hybrid::color::AlphaColor::<vello_hybrid::color::Srgb>::new([s.color.r, s.color.g, s.color.b, s.color.a,
                        ]).into(),
                    }).collect();
                let g = if gradient.angle.is_some() {
                    let (a,b) = gradient.line(*r);
                    Gradient::new_linear((a.x as f64, a.y as f64), (b.x as f64, b.y as f64))
                } else {
                    let c = r.center();
                    Gradient::new_radial((c.x as f64, c.y as f64), ((r.width() * r.width() + r.height() * r.height()).sqrt() * 0.5).max(0.001),
                    )
                }.with_stops(stops.as_slice());
                ctx.set_paint(g);
                let p = rounded(*r, *radius);
                if *width > 0.0 { ctx.set_stroke(Stroke::new(*width as f64)); ctx.stroke_path(&p); }
                else { ctx.fill_path(&p); }
            }
            Op::Fill { rect: r, radius, color,
            } => path(ctx, &rounded(*r, *radius), 0., *color, false),
            Op::Stroke { rect: r, radius, width, color,
            } => path(ctx, &rounded(*r, *radius), *width, *color, false),
            Op::Continuous { rect: r, radius, smoothing, width, color,
            } => {
                let mut p = BezPath::new();
                for (i, v) in crate::geom::continuous::outline(*r, *radius, *smoothing).iter().enumerate() {
                    if i == 0 {
                        p.move_to((v.x as f64, v.y as f64));
                    } else {
                        p.line_to((v.x as f64, v.y as f64));
                    }
                }
                p.close_path();
                path(ctx, &p, *width, *color, false);
            }
            Op::Line { from, to, width, color,
            } => {
                let mut p = BezPath::new();
                p.move_to((from.x as f64, from.y as f64));
                p.line_to((to.x as f64, to.y as f64));
                path(ctx, &p, *width, *color, false);
            }
            Op::Arc { center, radius, start, sweep, width, color,
            } => {
                // Kurbo's angles run from three o'clock, clockwise in y-down space.
                let arc = kurbo::Arc::new(
                    (center.x as f64, center.y as f64),
                    (*radius as f64, *radius as f64),
                    -std::f64::consts::FRAC_PI_2 + *start as f64,
                    (*sweep as f64).min(std::f64::consts::TAU),
                    0.,
                );
                path(ctx, &arc.to_path(0.05), *width, *color, false);
            }
            Op::Icon { rect: r, icon, weight, color,
            } => {
                let p = Affine::translate((r.left as f64, r.top as f64))
                    * Affine::scale_non_uniform(r.width() as f64 / 24., r.height() as f64 / 24.)
                    * icon_path(*icon);
                if icon.filled() {
                    path(ctx, &p, 0., *color, true);
                }
                path(ctx, &p, *weight, *color, true);
            }
            Op::Text { rect: r, text: label, style, color,
            } | Op::Emphasis { rect: r, text: label, style, color, .. } => {
                if label.chars().count() == 1
                    && let Some(icon) = label.chars().next().and_then(crate::icons::from_system_glyph)
                {
                    let icon = Op::Icon { rect: Rect::new(r.left, r.top, style.size, style.size), icon, weight: (style.size / 12.).max(1.), color: *color,
                    };
                    draw_ops(ctx, res, text, &[icon], scene_transform, image_source)?;
                    continue;
                }
                let italic = matches!(op, Op::Emphasis { italic: true, .. });
                let layout = text.shape(label, style, italic);
                paint(ctx, *color);
                for (i, (ct, _)) in layout.lines.iter().enumerate() {
                    line(ctx, ct, (r.left as f64, r.top as f64 + layout.ascent + i as f64 * layout.line_height,
                        ),
                    );
                }
                if matches!(op, Op::Emphasis { strike: true, .. }) {
                    let y = (r.top + style.size * 0.65) as f64;
                    let mut p = BezPath::new();
                    p.move_to((r.left as f64, y));
                    p.line_to((r.right as f64, y));
                    path(ctx, &p, 1., *color, false);
                }
            }
            Op::Image { rect: r, image, opacity, source,
            } | Op::RoundedImage { rect: r, image, opacity, source, .. } => {
                ensure!(image.width <= u16::MAX as u32 && image.height <= u16::MAX as u32, "image too large");
                let mut pix = Pixmap::new(image.width as u16, image.height as u16);
                for (d, p) in pix.data_mut().iter_mut().zip(image.pixels.as_chunks::<4>().0.iter()) {
                    *d = vello_hybrid::color::PremulRgba8::from_u8_array([p[2], p[1], p[0], p[3]]);
                }
                let src = source.unwrap_or(Rect::new(0., 0., image.width as f32, image.height as f32));
                ctx.set_paint(vello_hybrid::Image { image: image_source(res, Arc::new(pix)), sampler: Default::default(),
                });
                ctx.set_paint_transform(
                    Affine::translate((r.left as f64, r.top as f64))
                        * Affine::scale_non_uniform((r.width() / src.width()) as f64, (r.height() / src.height()) as f64,
                        )
                        * Affine::translate((-src.left as f64, -src.top as f64)),
                );
                ctx.push_opacity_layer(*opacity);
                let radius = if let Op::RoundedImage { radius, .. } = op { *radius } else { 0. };
                ctx.fill_path(&rounded(*r, radius));
                ctx.pop_layer();
                ctx.reset_paint_transform();
            }
            Op::Shadow { rect: r, radius, spread, drop, color,
            } => {
                let steps = (*spread / 2.).floor() as i32;
                for i in 0..steps {
                    let grow = (i * 2) as f32;
                    path(ctx, &rounded(r.inflate(grow).offset(0., *drop), radius + grow), 0., color.with_alpha(color.a * (1. - i as f32 / steps as f32)), false,
                    );
                }
            }
            Op::SoftShadow { rect: r, radius, blur, offset, spread, color,
            } => {
                // As on Linux: Vello's exact blurred rounded rectangle.
                let (shape, radius) = crate::shadow::shape(*r, *radius, *offset, *spread);
                let radius = radius.min(shape.width() / 2.).min(shape.height() / 2.);
                let sigma = crate::shadow::sigma(*blur);
                if color.a > 0. && !shape.is_empty() {
                    if sigma < 0.25 {
                        path(ctx, &rounded(shape, radius), 0., *color, false);
                    } else {
                        paint(ctx, *color);
                        ctx.fill_blurred_rounded_rect(&rect(shape), radius, sigma, false);
                    }
                }
            }
            Op::TextField { rect, field, .. } => draw_ops(ctx, res, text, &[field.op(*rect)], scene_transform, image_source,
            )?,
        }
    }
    for _ in 0..clips {
        ctx.pop_layer();
    }
    Ok(())
}

fn icon_path(icon: crate::icons::Icon) -> BezPath {
    use crate::icons::Seg;
    let mut path = BezPath::new();
    let mut at = kurbo::Point::ZERO;
    let point = |p: crate::Point| kurbo::Point::new(p.x as f64, p.y as f64);
    for seg in icon.segments() {
        match seg {
            Seg::Move(p) => {
                at = point(p);
                path.move_to(at);
            }
            Seg::Line(p) => {
                at = point(p);
                path.line_to(at);
            }
            Seg::Quad(a, p) => {
                at = point(p);
                path.quad_to(point(a), at);
            }
            Seg::Cubic(a, b, p) => {
                at = point(p);
                path.curve_to(point(a), point(b), at);
            }
            Seg::Close => path.close_path(),
            Seg::Arc { to, rx, ry, rotation, large, sweep,
            } => {
                let to = point(to);
                let svg = kurbo::SvgArc {
                    from: at,
                    to,
                    radii: kurbo::Vec2::new(rx as f64, ry as f64),
                    x_rotation: (rotation as f64).to_radians(),
                    large_arc: large,
                    sweep,
                };
                match kurbo::Arc::from_svg_arc(&svg) {
                    Some(arc) => path.extend(arc.append_iter(0.05)),
                    None => path.line_to(to),
                }
                at = to;
            }
        }
    }
    path
}
