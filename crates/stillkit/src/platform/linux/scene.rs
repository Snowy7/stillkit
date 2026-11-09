use super::super::Text;
use crate::{Op, Rect, Rgba};
use anyhow::{Result, ensure};
use skrifa::MetadataProvider;
use std::sync::Arc;
use vello_cpu::{
    Pixmap,
    kurbo::{self, Affine, BezPath, Cap, Join, Shape, Stroke},
};
fn rect(r: Rect) -> kurbo::Rect {
    kurbo::Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)
}
fn rounded(r: Rect, radius: f32) -> BezPath {
    kurbo::RoundedRect::from_rect(rect(r), radius as f64).to_path(0.05)
}
fn paint(ctx: &mut Scene, c: Rgba) {
    ctx.set_paint(vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::new(
        [c.r, c.g, c.b, c.a],
    ));
}
fn path(ctx: &mut Scene, p: &BezPath, width: f32, c: Rgba) {
    paint(ctx, c);
    if width > 0. {
        ctx.set_stroke(
            Stroke::new(width as f64)
                .with_caps(Cap::Round)
                .with_join(Join::Round),
        );
        ctx.stroke_path(p);
    } else {
        ctx.fill_path(p);
    }
}
pub(super) fn draw_ops(
    ctx: &mut Scene,
    res: &mut Resources,
    text: &mut Text,
    ops: &[Op],
    scene_transform: Affine,
    image_source: &mut impl FnMut(&mut Resources, Arc<Pixmap>) -> vello_cpu::ImageSource,
) -> Result<()> {
    let mut clips = 0;
    for op in ops {
        match op {
            Op::Transformed { transform, ops } => {
                let saved = scene_transform;
                ctx.set_transform(saved * Affine::new(transform.0.map(f64::from)));
                let result = draw_ops(ctx,res,text,ops,saved * Affine::new(transform.0.map(f64::from)),image_source);
                ctx.set_transform(saved);
                result?;
            }
            Op::Hole { rect: r } => {
                ctx.set_blend_mode(vello_cpu::peniko::BlendMode::new(vello_cpu::peniko::Mix::Normal, vello_cpu::peniko::Compose::Copy));
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
            Op::RoundedClipped {
                rect: r,
                radius,
                ops,
            } => {
                ctx.push_clip_layer(&rounded(*r, *radius));
                draw_ops(ctx, res, text, ops, scene_transform, image_source)?;
                ctx.pop_layer();
            }
            Op::Gradient { rect: r, radius, width, gradient } => {
                use vello_cpu::peniko::{Gradient, ColorStop};
                let stops: Vec<_> = gradient.stops.iter().map(|s| ColorStop { offset: s.offset, color: vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::new([s.color.r, s.color.g, s.color.b, s.color.a]).into() }).collect();
                let g = if gradient.angle.is_some() {
                    let (a,b) = gradient.line(*r);
                    Gradient::new_linear((a.x as f64, a.y as f64), (b.x as f64, b.y as f64))
                } else {
                    let c = r.center();
                    Gradient::new_radial((c.x as f64, c.y as f64), ((r.width() * r.width() + r.height() * r.height()).sqrt() * 0.5).max(0.001))
                }.with_stops(stops.as_slice());
                ctx.set_paint(g);
                let p = rounded(*r, *radius);
                if *width > 0.0 { ctx.set_stroke(Stroke::new(*width as f64).with_caps(Cap::Round).with_join(Join::Round)); ctx.stroke_path(&p); }
                else { ctx.fill_path(&p); }
            }
            Op::Fill {
                rect: r,
                radius,
                color,
            } => path(ctx, &rounded(*r, *radius), 0., *color),
            Op::Stroke {
                rect: r,
                radius,
                width,
                color,
            } => path(ctx, &rounded(*r, *radius), *width, *color),
            Op::Continuous {
                rect: r,
                radius,
                smoothing,
                width,
                color,
            } => {
                let points = crate::geom::continuous::outline(*r, *radius, *smoothing);
                let mut p = BezPath::new();
                for (i, v) in points.iter().enumerate() {
                    if i == 0 {
                        p.move_to((v.x as f64, v.y as f64));
                    } else {
                        p.line_to((v.x as f64, v.y as f64));
                    }
                }
                p.close_path();
                path(ctx, &p, *width, *color);
            }
            Op::Line {
                from,
                to,
                width,
                color,
            } => {
                let mut p = BezPath::new();
                p.move_to((from.x as f64, from.y as f64));
                p.line_to((to.x as f64, to.y as f64));
                path(ctx, &p, *width, *color);
            }
            Op::Arc {
                center,
                radius,
                start,
                sweep,
                width,
                color,
            } => {
                // Kurbo's angles run from three o'clock, clockwise in y-down space.
                let arc = kurbo::Arc::new(
                    (center.x as f64, center.y as f64),
                    (*radius as f64, *radius as f64),
                    -std::f64::consts::FRAC_PI_2 + *start as f64,
                    *sweep as f64,
                    0.,
                );
                path(ctx, &arc.to_path(0.05), *width, *color);
            }
            Op::Icon {
                rect: r,
                icon,
                weight,
                color,
            } => {
                let p = icon_path(*icon);
                let p = Affine::translate((r.left as f64, r.top as f64))
                    * Affine::scale_non_uniform(r.width() as f64 / 24., r.height() as f64 / 24.)
                    * p;
                if icon.filled() {
                    path(ctx, &p, 0., *color);
                }
                path(ctx, &p, *weight, *color);
            }
            Op::Text {
                rect: r,
                text: label,
                style,
                color,
            }
            | Op::Emphasis {
                rect: r,
                text: label,
                style,
                color,
                ..
            } => {
                if label.chars().count() == 1
                    && let Some(icon) = label
                        .chars()
                        .next()
                        .and_then(crate::icons::from_system_glyph)
                {
                    draw_ops(
                        ctx,
                        res,
                        text,
                        &[Op::Icon {
                            rect: Rect::new(r.left, r.top, style.size, style.size),
                            icon,
                            weight: (style.size / 12.).max(1.),
                            color: *color,
                        }],
                        image_source,
                    )?;
                    continue;
                }
                let italic = matches!(op, Op::Emphasis { italic: true, .. });
                let glyphs: Vec<_> = text
                    .shape_emphasis(label, style, italic)
                    .layout_runs()
                    .flat_map(|run| run.glyphs.iter().map(move |g| (g.clone(), run.line_y)))
                    .collect();
                paint(ctx, *color);
                for (g, y) in glyphs {
                    if let Some(font) = text.fonts.get_font(g.font_id, g.font_weight) {
                        let face = font.as_peniko();
                        let location = skrifa::FontRef::from_index(font.data(), face.index)?
                            .axes()
                            .location([("wght", g.font_weight.0 as f32)]);
                        let coords: Vec<_> =
                            location.coords().iter().map(|c| c.to_bits()).collect();
                        ctx.glyph_run(res, &face)
                            .font_size(g.font_size)
                            .normalized_coords(&coords)
                            .atlas_cache(false)
                            .fill_glyphs(
                                [vello_cpu::Glyph {
                                    id: g.glyph_id as u32,
                                    x: r.left + g.x + g.x_offset,
                                    y: r.top + y - g.y_offset,
                                }]
                                .into_iter(),
                            )?;
                    }
                }
                if matches!(op, Op::Emphasis { strike: true, .. }) {
                    let y = r.top + style.size * 0.65;
                    let mut p = BezPath::new();
                    p.move_to((r.left as f64, y as f64));
                    p.line_to((r.right as f64, y as f64));
                    path(ctx, &p, 1., *color);
                }
            }
            Op::Image {
                rect: r,
                image,
                opacity,
                source,
            }
            | Op::RoundedImage {
                rect: r,
                image,
                opacity,
                source,
                ..
            } => {
                ensure!(
                    image.width <= u16::MAX as u32 && image.height <= u16::MAX as u32,
                    "image too large"
                );
                let mut pix = Pixmap::new(image.width as u16, image.height as u16);
                // data_mut exposes premultiplied RGBA components.
                for (d, p) in pix
                    .data_mut()
                    .iter_mut()
                    .zip(image.pixels.as_chunks::<4>().0.iter())
                {
                    *d = vello_cpu::color::PremulRgba8::from_u8_array([p[2], p[1], p[0], p[3]]);
                }
                let src =
                    source.unwrap_or(Rect::new(0., 0., image.width as f32, image.height as f32));
                ctx.set_paint(vello_cpu::Image {
                    image: image_source(res, Arc::new(pix)),
                    sampler: Default::default(),
                });
                ctx.set_paint_transform(
                    Affine::translate((r.left as f64, r.top as f64))
                        * Affine::scale_non_uniform(
                            (r.width() / src.width()) as f64,
                            (r.height() / src.height()) as f64,
                        )
                        * Affine::translate((-src.left as f64, -src.top as f64)),
                );
                ctx.push_opacity_layer(*opacity);
                ctx.fill_path(&rounded(
                    *r,
                    if let Op::RoundedImage { radius, .. } = op {
                        *radius
                    } else {
                        0.
                    },
                ));
                ctx.pop_layer();
                ctx.reset_paint_transform();
            }
            Op::Shadow {
                rect: r,
                radius,
                spread,
                drop,
                color,
            } => {
                let steps = (*spread / 2.).floor() as i32;
                for i in 0..steps {
                    let grow = (i * 2) as f32;
                    path(
                        ctx,
                        &rounded(r.inflate(grow).offset(0., *drop), radius + grow),
                        0.,
                        color.with_alpha(color.a * (1. - i as f32 / steps as f32)),
                    );
                }
            }
            Op::SoftShadow {
                rect: r,
                radius,
                blur,
                offset,
                spread,
                color,
            } => {
                let (shape, radius) = crate::shadow::shape(*r, *radius, *offset, *spread);
                let radius = radius.min(shape.width() / 2.).min(shape.height() / 2.);
                let sigma = crate::shadow::sigma(*blur);
                if color.a > 0. && !shape.is_empty() {
                    if sigma < 0.25 {
                        path(ctx, &rounded(shape, radius), 0., *color);
                    } else {
                        paint(ctx, *color);
                        ctx.fill_blurred_rounded_rect(&rect(shape), radius, sigma, false);
                    }
                }
            }
            Op::TextField { rect, field, .. } => {
                draw_ops(ctx, res, text, &[field.op(*rect)], scene_transform, image_source)?;
            }
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
            Seg::Arc {
                to,
                rx,
                ry,
                rotation,
                large,
                sweep,
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
                if let Some(arc) = kurbo::Arc::from_svg_arc(&svg) {
                    path.extend(arc.append_iter(0.05));
                } else {
                    path.line_to(to);
                }
                at = to;
            }
        }
    }
    path
}
