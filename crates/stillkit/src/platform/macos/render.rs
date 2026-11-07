//! CoreGraphics into a `CGBitmapContext`: premultiplied BGRA (the crate's [`Image`] layout),
//! for live surfaces and headless renders and goldens alike. Only the damaged bounds are
//! cleared and drawn; text goes through CoreText. No Metal device is made here.
use super::Text;
pub use crate::bitmap::{Image, TextMode, TextQuality};
use crate::{DisplayList, PixelRect};
use anyhow::{Context, Result, bail, ensure};
use objc2_core_foundation::{CFData, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetBytesPerRow, CGBitmapContextGetData,
    CGColorSpace, CGContext, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo, kCGColorSpaceSRGB,
};
use objc2_image_io::{
    CGImageSource, kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform,
    kCGImageSourceThumbnailMaxPixelSize,
};
use std::cell::RefCell;

/// Per-thread text and image services, usable without a window (any thread).
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
    /// CoreText rasterizes grayscale with its own tuning; DirectWrite's knobs are Windows-only.
    pub fn set_text_quality(&mut self, _: TextQuality) -> Result<()> {
        Ok(())
    }
    /// Decodes with ImageIO (PNG, JPEG, HEIC...), turned upright and fitted within `max`.
    pub fn decode_image(&self, bytes: &[u8], max: (u32, u32)) -> Result<Image> {
        let data = CFData::from_bytes(bytes);
        // SAFETY: a CFData of the caller's bytes; no options.
        let source = unsafe { CGImageSource::with_data(&data, None) }.context("ImageIO: unreadable image")?;
        let (always, transform, size_key) = unsafe {
            // SAFETY: extern statics ImageIO defines.
            (
                kCGImageSourceCreateThumbnailFromImageAlways,
                kCGImageSourceCreateThumbnailWithTransform,
                kCGImageSourceThumbnailMaxPixelSize,
            )
        };
        let yes = objc2_core_foundation::CFBoolean::new(true);
        let limit = CFNumber::new_i64(i64::from(max.0.max(max.1).max(1)));
        let options = CFDictionary::<CFString, CFType>::from_slices(
            &[always, transform, size_key],
            &[yes.as_ref(), yes.as_ref(), limit.as_ref()],
        );
        // SAFETY: a live source; options of ImageIO's own keys.
        let image = unsafe { source.thumbnail_at_index(0, Some(options.as_opaque())) }
            .context("ImageIO: no image in the data")?;
        let (w, h) = (CGImage::width(Some(&image)) as f64, CGImage::height(Some(&image)) as f64);
        // The thumbnail fits the larger bound; fit both, never enlarging.
        let fit = (max.0 as f64 / w).min(max.1 as f64 / h).min(1.0);
        let (width, height) = (((w * fit).round() as u32).max(1), ((h * fit).round() as u32).max(1));
        let canvas = Canvas::new(self, width, height)?;
        // The context's top-left user space draws images upside down: flip locally.
        CGContext::save_g_state(Some(&canvas.ctx));
        CGContext::translate_ctm(Some(&canvas.ctx), 0.0, height as f64);
        CGContext::scale_ctm(Some(&canvas.ctx), 1.0, -1.0);
        CGContext::draw_image(Some(&canvas.ctx), CGRect::new(Default::default(), objc2_core_foundation::CGSize::new(width as f64, height as f64)), Some(&image));
        CGContext::restore_g_state(Some(&canvas.ctx));
        Ok(canvas.image())
    }
}

impl Image {
    pub fn load(ui: &Ui, path: &std::path::Path) -> Result<Self> {
        ui.decode_image(&std::fs::read(path)?, (32768, 32768))
    }
}

/// Rasterizes a display list headlessly to premultiplied BGRA pixels: CoreGraphics, or Vello
/// Hybrid on Metal when this thread renders on the GPU (`gpu` feature).
pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
    #[cfg(feature = "gpu")]
    if crate::platform::render().resolve(crate::glass::wanted(), crate::glass::backdrop().is_some())
        == crate::platform::Render::Gpu
    {
        return super::gpu::render(ui, list, width, height);
    }
    let canvas = Canvas::new(ui, width, height)?;
    canvas.draw(&mut ui.text, list, PixelRect::full(width, height))?;
    Ok(canvas.image())
}

/// A retained premultiplied BGRA bitmap context; draw and read only damaged bounds.
pub struct Canvas {
    pub(super) ctx: CFRetained<CGContext>,
    size: (u32, u32),
    /// Images drawn in the last frame, made into CGImages once.
    images: RefCell<super::draw::Images>,
}

/// sRGB, premultiplied alpha first, 32-bit little endian: B, G, R, A in memory.
pub(super) fn bgra() -> u32 {
    CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0
}

pub(super) fn srgb() -> CFRetained<CGColorSpace> {
    // SAFETY: an extern static CoreGraphics defines.
    CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).expect("sRGB colour space")
}

impl Canvas {
    pub fn new(_: &Ui, width: u32, height: u32) -> Result<Self> {
        ensure!(width > 0 && height > 0 && width <= 32768 && height <= 32768, "invalid bitmap size");
        // SAFETY: CoreGraphics allocates (and zeroes) the pixels; the row size fits the width.
        let ctx = unsafe {
            CGBitmapContextCreate(std::ptr::null_mut(), width as usize, height as usize, 8, width as usize * 4, Some(&srgb()), bgra())
        };
        let Some(ctx) = ctx else { bail!("CGBitmapContextCreate {width}x{height}") };
        super::draw::prepare(&ctx, (width, height));
        Ok(Self { ctx, size: (width, height), images: RefCell::default() })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The bitmap's rows, top first, `width * 4` bytes each.
    fn rows(&self) -> (&[u8], usize) {
        let stride = CGBitmapContextGetBytesPerRow(Some(&self.ctx));
        let data = CGBitmapContextGetData(Some(&self.ctx)).cast::<u8>();
        // SAFETY: the context owns `stride * height` bytes for its life, which `&self` holds.
        (unsafe { std::slice::from_raw_parts(data, stride * self.size.1 as usize) }, stride)
    }

    pub fn image(&self) -> Image {
        let mut pixels = Vec::with_capacity(self.size.0 as usize * self.size.1 as usize * 4);
        let _ = self.read(PixelRect::full(self.size.0, self.size.1), |_, row| pixels.extend_from_slice(row));
        Image { width: self.size.0, height: self.size.1, pixels }
    }

    /// The bitmap as a CGImage for a layer's contents (copy-on-write: the next draw copies
    /// only the pages it touches).
    pub(super) fn cg_image(&self) -> Option<CFRetained<CGImage>> {
        CGBitmapContextCreateImage(Some(&self.ctx))
    }

    pub fn draw(&self, text: &mut Text, list: &DisplayList, area: PixelRect) -> Result<()> {
        ensure!(area.x + area.width <= self.size.0 && area.y + area.height <= self.size.1, "damage outside bitmap");
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        super::draw::region(&self.ctx, text, &mut self.images.borrow_mut(), list, area);
        Ok(())
    }

    pub fn read(&self, area: PixelRect, mut sink: impl FnMut(u32, &[u8])) -> Result<()> {
        ensure!(area.x + area.width <= self.size.0 && area.y + area.height <= self.size.1, "read outside bitmap");
        let (data, stride) = self.rows();
        for y in 0..area.height {
            let i = (area.y + y) as usize * stride + area.x as usize * 4;
            sink(y, &data[i..i + area.width as usize * 4]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Op, Rect, Rgba};
    use std::sync::Arc;

    #[test]
    fn rounded_groups_clip_and_translate_without_losing_the_mask() {
        let mut ui = Ui::new().unwrap();
        let fill = |color| Op::Fill { rect: Rect::new(0.0, 0.0, 40.0, 40.0), radius: 0.0, color: Rgba::rgb8(color, 1.0) };
        let list = DisplayList {
            ops: vec![Op::RoundedClipped {
                rect: Rect::new(4.0, 4.0, 24.0, 28.0),
                radius: 8.0,
                ops: vec![fill([255, 0, 0]), Op::Clipped { rect: Rect::new(4.0, 4.0, 24.0, 14.0), ops: vec![fill([0, 0, 255])].into() }].into(),
            }],
            ..Default::default()
        };
        let image = render(&mut ui, &list, 40, 40).unwrap();
        assert_eq!(image.pixel(4, 4), [0, 0, 0, 0]);
        assert_eq!(image.pixel(16, 4), [255, 0, 0, 255]);
        assert_eq!(image.pixel(4, 18), [0, 0, 255, 255]);
        let translated = render(&mut ui, &list.translated(3.0, 2.0), 48, 48).unwrap();
        for y in 0..40 {
            for x in 0..40 {
                assert_eq!(translated.pixel(x + 3, y + 2), image.pixel(x, y));
            }
        }
    }

    #[test]
    fn damage_patch_matches_full_render_and_preserves_untouched_pixels() {
        let mut ui = Ui::new().unwrap();
        let frame = |color| DisplayList {
            ops: vec![
                Op::Fill { rect: Rect::new(2., 2., 60., 44.), radius: 12., color },
                Op::Text { rect: Rect::new(6., 8., 50., 20.), text: "Hello".into(), style: crate::TextStyle::new(super::super::text::SYSTEM, 13.), color: Rgba::new(1., 1., 1., 1.) },
            ],
            ..Default::default()
        };
        let canvas = Canvas::new(&ui, 64, 48).unwrap();
        canvas.draw(&mut ui.text, &frame(Rgba::new(1., 0., 0., 0.5)), PixelRect::full(64, 48)).unwrap();
        let old = canvas.image();
        let after = frame(Rgba::new(0., 0., 1., 0.5));
        canvas.draw(&mut ui.text, &after, PixelRect { x: 7, y: 3, width: 29, height: 25 }).unwrap();
        let (actual, full) = (canvas.image(), render(&mut ui, &after, 64, 48).unwrap());
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

    #[test]
    fn bitmap_ops_keep_premultiplied_alpha_and_cover_only_their_rect() {
        let mut ui = Ui::new().unwrap();
        let list = DisplayList {
            ops: vec![Op::Image {
                rect: Rect::new(1.0, 1.0, 2.0, 2.0),
                image: Arc::new(Image { width: 1, height: 1, pixels: vec![0, 0, 128, 128] }),
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
    fn semibold_is_between_regular_and_bold() {
        let mut ui = Ui::new().unwrap();
        let ink: Vec<u64> = [400, 600, 700]
            .into_iter()
            .map(|weight| {
                let style = crate::TextStyle { weight, ..crate::TextStyle::new(super::super::text::SYSTEM, 24.) };
                let list = DisplayList {
                    ops: vec![Op::Text { rect: Rect::new(0., 0., 250., 40.), text: "Semibold specimen".into(), style, color: Rgba::new(1., 1., 1., 1.) }],
                    ..Default::default()
                };
                render(&mut ui, &list, 280, 40).unwrap().pixels.as_chunks::<4>().0.iter().map(|p| p[3] as u64).sum()
            })
            .collect();
        assert!(ink[0] < ink[1] && ink[1] < ink[2], "400/600/700 ink coverage: {ink:?}");
    }
}
