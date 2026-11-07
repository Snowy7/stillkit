//! CPU-only Vello renderer. The glyph bitmap atlas is deliberately disabled.
use super::Text;
pub use crate::bitmap::{Image, TextMode, TextQuality};
use crate::{DisplayList, Op, PixelRect, Rect, Rgba};
use anyhow::{Result, ensure};
use skrifa::MetadataProvider;
use std::{cell::RefCell, sync::Arc};
use vello_cpu::{
    Pixmap, RenderContext, Resources,
    kurbo::{self, Affine, BezPath, Cap, Join, Shape, Stroke},
};
/// Per-thread text and image services, usable without a Wayland connection.
pub struct Ui {
    pub text: Text,
}
impl Ui {
    pub fn new() -> Result<Self> {
        Ok(Self { text: Text::new()? })
    }
    pub fn isolated() -> Result<Self> {
        Self::new()
    }
    /// Linux uses Vello's grayscale outline rasterizer; DirectWrite tuning is Windows-only.
    pub fn set_text_quality(&mut self, _: TextQuality) -> Result<()> {
        Ok(())
    }
    pub fn decode_image(&self, bytes: &[u8], max: (u32, u32)) -> Result<Image> {
        let decoded = image::load_from_memory(bytes)?
            .thumbnail(max.0, max.1)
            .to_rgba8();
        let (width, height) = decoded.dimensions();
        let mut pixels = decoded.into_raw();
        for p in pixels.as_chunks_mut::<4>().0.iter_mut() {
            let a = p[3] as u16;
            let r = p[0];
            p[0] = (p[2] as u16 * a / 255) as u8;
            p[1] = (p[1] as u16 * a / 255) as u8;
            p[2] = (r as u16 * a / 255) as u8;
        }
        Ok(Image {
            width,
            height,
            pixels,
        })
    }
}
impl Image {
    pub fn load(ui: &Ui, path: &std::path::Path) -> Result<Self> {
        ui.decode_image(&std::fs::read(path)?, (32768, 32768))
    }
}
/// Rasterizes a display list headlessly to premultiplied BGRA pixels.
pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
    let canvas = Canvas::new(ui, width, height)?;
    canvas.draw(&mut ui.text, list, PixelRect::full(width, height))?;
    Ok(canvas.image())
}
struct State {
    resources: Resources,
    pixels: Image,
}
/// A retained premultiplied bitmap; draw and read only damaged bounds.
pub struct Canvas {
    state: RefCell<State>,
}
impl Canvas {
    pub fn new(_: &Ui, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= u16::MAX as u32 && height <= u16::MAX as u32,
            "invalid bitmap size"
        );
        Ok(Self {
            state: RefCell::new(State {
                resources: Resources::new(),
                pixels: Image {
                    width,
                    height,
                    pixels: vec![0; width as usize * height as usize * 4],
                },
            }),
        })
    }
    pub fn size(&self) -> (u32, u32) {
        let s = self.state.borrow();
        (s.pixels.width, s.pixels.height)
    }
    pub fn image(&self) -> Image {
        self.state.borrow().pixels.clone()
    }
    pub fn draw(&self, text: &mut Text, list: &DisplayList, area: PixelRect) -> Result<()> {
        let mut s = self.state.borrow_mut();
        ensure!(
            area.x + area.width <= s.pixels.width && area.y + area.height <= s.pixels.height,
            "damage outside bitmap"
        );
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        // Rasterize a damage-sized target, rather than a full bitmap under a scissor.
        let mut ctx = RenderContext::new(area.width as u16, area.height as u16);
        ctx.set_transform(Affine::translate((-(area.x as f64), -(area.y as f64))));
        draw_ops(&mut ctx, &mut s.resources, text, &list.ops)?;
        ctx.flush();
        let mut target = Pixmap::new(area.width as u16, area.height as u16);
        ctx.render(&mut target, &mut s.resources);
        for y in 0..area.height {
            let dst = ((area.y + y) * s.pixels.width + area.x) as usize * 4;
            let src = y as usize * area.width as usize * 4;
            for (d, p) in s.pixels.pixels[dst..dst + area.width as usize * 4]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(
                    target.data_as_u8_slice()[src..src + area.width as usize * 4]
                        .as_chunks::<4>()
                        .0
                        .iter(),
                )
            {
                d.copy_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        Ok(())
    }
    pub fn read(&self, area: PixelRect, mut sink: impl FnMut(u32, &[u8])) -> Result<()> {
        let s = self.state.borrow();
        ensure!(
            area.x + area.width <= s.pixels.width && area.y + area.height <= s.pixels.height,
            "read outside bitmap"
        );
        for y in 0..area.height {
            let i = ((area.y + y) * s.pixels.width + area.x) as usize * 4;
            sink(y, &s.pixels.pixels[i..i + area.width as usize * 4]);
        }
        Ok(())
    }
}
impl crate::platform::Renderer for Canvas {
    type Context = Ui;
    fn new(ui: &Ui, w: u32, h: u32) -> Result<Self> {
        Self::new(ui, w, h)
    }
    fn draw(&self, t: &mut Text, l: &DisplayList, a: PixelRect) -> Result<()> {
        self.draw(t, l, a)
    }
    fn read(&self, a: PixelRect, s: impl FnMut(u32, &[u8])) -> Result<()> {
        self.read(a, s)
    }
}
fn rect(r: Rect) -> kurbo::Rect {
    kurbo::Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)
}
fn rounded(r: Rect, radius: f32) -> BezPath {
    kurbo::RoundedRect::from_rect(rect(r), radius as f64).to_path(0.05)
}
fn paint(ctx: &mut RenderContext, c: Rgba) {
    ctx.set_paint(vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::new(
        [c.r, c.g, c.b, c.a],
    ));
}
fn path(ctx: &mut RenderContext, p: &BezPath, width: f32, c: Rgba) {
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
fn draw_ops(
    ctx: &mut RenderContext,
    res: &mut Resources,
    text: &mut Text,
    ops: &[Op],
) -> Result<()> {
    let mut clips = 0;
    for op in ops {
        match op {
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
                draw_ops(ctx, res, text, ops)?;
                ctx.pop_layer();
            }
            Op::RoundedClipped {
                rect: r,
                radius,
                ops,
            } => {
                ctx.push_clip_layer(&rounded(*r, *radius));
                draw_ops(ctx, res, text, ops)?;
                ctx.pop_layer();
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
                sweep,
                width,
                color,
            } => {
                let arc = kurbo::Arc::new(
                    (center.x as f64, center.y as f64),
                    (*radius as f64, *radius as f64),
                    -std::f64::consts::FRAC_PI_2,
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
                let transform = *ctx.transform();
                ctx.set_paint(vello_cpu::Image {
                    image: vello_cpu::ImageSource::Pixmap(Arc::new(pix)),
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
                ctx.set_transform(transform);
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
            Op::TextField { rect, field, .. } => {
                draw_ops(ctx, res, text, &[field.op(*rect)])?;
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
#[cfg(test)]
#[test]
fn rounded_groups_clip_composed_pictures_and_translate_without_losing_the_mask() {
    let mut ui = Ui::new().unwrap();
    let list = DisplayList {
        ops: vec![Op::RoundedClipped {
            rect: Rect::new(4.0, 4.0, 24.0, 28.0),
            radius: 8.0,
            ops: vec![
                Op::Fill {
                    rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                    radius: 0.0,
                    color: Rgba::rgb8([255, 0, 0], 1.0),
                },
                Op::Clipped {
                    rect: Rect::new(4.0, 4.0, 24.0, 14.0),
                    ops: vec![Op::Fill {
                        rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                        radius: 0.0,
                        color: Rgba::rgb8([0, 0, 255], 1.0),
                    }]
                    .into(),
                },
            ]
            .into(),
        }],
        ..Default::default()
    };
    let image = render(&mut ui, &list, 40, 40).unwrap();
    assert_eq!(image.pixel(4, 4), [0, 0, 0, 0]);
    assert_eq!(image.pixel(16, 4), [255, 0, 0, 255]);
    assert_eq!(
        image.pixel(4, 17),
        [255, 0, 0, 255],
        "the picture's lower edge stays square"
    );
    assert_eq!(image.pixel(4, 18), [0, 0, 255, 255]);
    assert_eq!(image.pixel(4, 31), [0, 0, 0, 0]);
    let translated = render(&mut ui, &list.translated(3.0, 2.0), 48, 48).unwrap();
    for y in 0..40 {
        for x in 0..40 {
            assert_eq!(translated.pixel(x + 3, y + 2), image.pixel(x, y));
        }
    }
}

#[cfg(test)]
#[test]
fn marker_and_retained_clips_are_nested_and_balanced() {
    let mut ui = Ui::new().unwrap();
    let fill = |color| Op::Fill {
        rect: Rect::new(0.0, 0.0, 32.0, 32.0),
        radius: 0.0,
        color: Rgba::rgb8(color, 1.0),
    };
    let list = DisplayList {
        ops: vec![
            Op::Clip {
                rect: Rect::new(0.0, 0.0, 12.0, 12.0),
            },
            Op::Clipped {
                rect: Rect::new(4.0, 4.0, 20.0, 20.0),
                ops: vec![
                    fill([255, 0, 0]),
                    Op::Clip {
                        rect: Rect::new(8.0, 8.0, 20.0, 20.0),
                    },
                    fill([0, 255, 0]),
                    Op::Unclip,
                    Op::Unclip,
                ]
                .into(),
            },
            Op::Unclip,
            Op::Fill {
                rect: Rect::new(24.0, 24.0, 4.0, 4.0),
                radius: 0.0,
                color: Rgba::rgb8([0, 0, 255], 1.0),
            },
        ],
        ..Default::default()
    };
    let image = render(&mut ui, &list, 32, 32).unwrap();
    assert_eq!(image.pixel(5, 5), [0, 0, 255, 255]);
    assert_eq!(image.pixel(10, 10), [0, 255, 0, 255]);
    assert_eq!(image.pixel(16, 16), [0, 0, 0, 0]);
    assert_eq!(image.pixel(25, 25), [255, 0, 0, 255]);
    let translated = render(&mut ui, &list.translated(3.0, 2.0), 40, 40).unwrap();
    for y in 0..32 {
        for x in 0..32 {
            assert_eq!(translated.pixel(x + 3, y + 2), image.pixel(x, y));
        }
    }
}

#[cfg(test)]
#[test]
fn damage_patch_matches_full_render_and_preserves_untouched_pixels() {
    let mut ui = Ui::new().unwrap();
    let frame = |color| DisplayList {
        ops: vec![
            Op::Fill {
                rect: Rect::new(2., 2., 60., 44.),
                radius: 12.,
                color,
            },
            Op::Text {
                rect: Rect::new(6., 8., 50., 20.),
                text: "Hello".into(),
                style: crate::TextStyle::new("Inter", 13.),
                color: Rgba::new(1., 1., 1., 1.),
            },
        ],
        ..Default::default()
    };
    let before = frame(Rgba::new(1., 0., 0., 0.5));
    let after = frame(Rgba::new(0., 0., 1., 0.5));
    let canvas = Canvas::new(&ui, 64, 48).unwrap();
    canvas
        .draw(&mut ui.text, &before, PixelRect::full(64, 48))
        .unwrap();
    let old = canvas.image();
    let damage = PixelRect {
        x: 7,
        y: 3,
        width: 29,
        height: 25,
    };
    canvas.draw(&mut ui.text, &after, damage).unwrap();
    let actual = canvas.image();
    let full = render(&mut ui, &after, 64, 48).unwrap();
    for y in 0..48 {
        for x in 0..64 {
            if (7..36).contains(&x) && (3..28).contains(&y) {
                for (a, b) in actual.pixel(x, y).into_iter().zip(full.pixel(x, y)) {
                    assert!(a.abs_diff(b) <= 1, "pixel {x},{y}: {a} vs {b}");
                }
            } else {
                assert_eq!(actual.pixel(x, y), old.pixel(x, y));
            }
        }
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    #[test]
    fn bitmap_ops_preserve_premultiplied_alpha_and_only_cover_their_rect() {
        let mut ui = Ui::new().unwrap();
        let bitmap = Arc::new(Image {
            width: 1,
            height: 1,
            pixels: vec![0, 0, 128, 128],
        });
        let list = DisplayList {
            ops: vec![Op::Image {
                rect: Rect::new(1.0, 1.0, 2.0, 2.0),
                image: bitmap,
                opacity: 1.0,
                source: None,
            }],
            ..Default::default()
        };
        let image = render(&mut ui, &list, 4, 4).unwrap();
        assert_eq!(image.pixel(0, 0), [0, 0, 0, 0]);
        let [b, g, r, a] = image.pixel(1, 1);
        assert_eq!((b, g), (0, 0));
        assert!(r.abs_diff(128) <= 1 && a.abs_diff(128) <= 1);
    }

    #[test]
    fn a_rounded_image_clears_its_corners_and_maps_its_source_crop() {
        let mut ui = Ui::new().unwrap();
        // Left half red, right half blue; the op shows only the right half, rounded.
        let pixels = (0..8 * 8)
            .flat_map(|i| {
                if i % 8 < 4 {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                }
            })
            .collect();
        let list = DisplayList {
            ops: vec![Op::RoundedImage {
                rect: Rect::new(0.0, 0.0, 32.0, 32.0),
                image: Arc::new(Image {
                    width: 8,
                    height: 8,
                    pixels,
                }),
                opacity: 1.0,
                source: Some(Rect::new(4.0, 0.0, 4.0, 8.0)),
                radius: 10.0,
            }],
            ..Default::default()
        };
        let image = render(&mut ui, &list, 32, 32).unwrap();
        assert_eq!(
            image.pixel(0, 0),
            [0, 0, 0, 0],
            "the corner is clipped clean"
        );
        assert_eq!(
            image.pixel(16, 16),
            [255, 0, 0, 255],
            "the centre shows the cropped half"
        );
        assert_eq!(
            image.pixel(6, 16),
            [255, 0, 0, 255],
            "the crop's left side is blue too"
        );
    }
}

#[cfg(test)]
#[test]
fn variable_font_semibold_is_between_regular_and_bold() {
    let mut ui = Ui::new().unwrap();
    let ink: Vec<u64> = [400, 600, 700]
        .into_iter()
        .map(|weight| {
            let list = DisplayList {
                ops: vec![Op::Text {
                    rect: Rect::new(0., 0., 250., 40.),
                    text: "Semibold specimen".into(),
                    style: crate::TextStyle {
                        weight,
                        ..crate::TextStyle::new("Inter", 24.)
                    },
                    color: Rgba::new(1., 1., 1., 1.),
                }],
                ..Default::default()
            };
            render(&mut ui, &list, 280, 40)
                .unwrap()
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3] as u64)
                .sum()
        })
        .collect();
    assert!(
        ink[0] < ink[1] && ink[1] < ink[2],
        "400/600/700 ink coverage: {ink:?}"
    );
}
