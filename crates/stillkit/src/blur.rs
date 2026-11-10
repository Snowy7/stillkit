//! Damage-sized CPU filters. Storage is allocated only when a scene uses blur.
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
use crate::platform::Renderer;
use crate::{DisplayList, Image, Op, PixelRect, Rect};
use anyhow::Result;
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, sync::Arc};

pub(crate) fn reach(sigma: f32) -> f32 {
    (sigma.clamp(0.0, 64.0) * 3.0).ceil()
}

struct Entry {
    source: Vec<Op>,
    rect: Rect,
    sigma: f32,
    mask: Option<(Rect, f32, f32)>,
    image: Arc<Image>,
}
thread_local! {
    static CACHE: RefCell<VecDeque<Entry>> = const { RefCell::new(VecDeque::new()) };
    static UI: RefCell<Option<crate::Ui>> = const { RefCell::new(None) };
}

fn raster(
    text: &mut crate::Text,
    source: Vec<Op>,
    rect: Rect,
    sigma: f32,
    mask: Option<(Rect, f32, f32)>,
) -> Result<Arc<Image>> {
    let sigma = if sigma.is_finite() {
        sigma.clamp(0.0, 64.0)
    } else {
        0.0
    };
    if let Some(image) = CACHE.with(|c| {
        c.borrow()
            .iter()
            .find(|e| e.rect == rect && e.sigma == sigma && e.mask == mask && e.source == source)
            .map(|e| e.image.clone())
    }) {
        return Ok(image);
    }
    let (width, height) = (rect.width() as u32, rect.height() as u32);
    let canvas = UI.with(|slot| -> Result<_> {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(crate::Ui::new()?);
        }
        crate::platform::current::Renderer::new(
            slot.as_ref().expect("initialized filter services"),
            width,
            height,
        )
    })?;
    let list = DisplayList {
        ops: source.clone(),
        ..Default::default()
    }
    .translated(-rect.left, -rect.top);
    canvas.draw(text, &list, PixelRect::full(width, height))?;
    let mut pixels = vec![0; width as usize * height as usize * 4];
    canvas.read(PixelRect::full(width, height), |y, row| {
        let i = y as usize * width as usize * 4;
        pixels[i..i + row.len()].copy_from_slice(row);
    })?;
    let original = mask
        .filter(|(_, radius, opacity)| *radius > 0.0 || *opacity < 1.0)
        .map(|_| pixels.clone());
    boxes(&mut pixels, width as usize, height as usize, sigma);
    if let (Some((bounds, radius, opacity)), Some(original)) = (mask, original) {
        let radius = radius.clamp(0.0, bounds.width().min(bounds.height()) * 0.5);
        for y in 0..height {
            for x in 0..width {
                let dx = (rect.left + x as f32 + 0.5 - (bounds.left + bounds.right) * 0.5).abs()
                    - bounds.width() * 0.5
                    + radius;
                let dy = (rect.top + y as f32 + 0.5 - (bounds.top + bounds.bottom) * 0.5).abs()
                    - bounds.height() * 0.5
                    + radius;
                let distance = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt() - radius
                } else {
                    dx.max(dy) - radius
                };
                let amount = (0.5 - distance).clamp(0.0, 1.0) * opacity;
                let i = ((y * width + x) * 4) as usize;
                for channel in 0..4 {
                    pixels[i + channel] = (original[i + channel] as f32 * (1.0 - amount)
                        + pixels[i + channel] as f32 * amount)
                        .round() as u8;
                }
            }
        }
    }
    let image = Arc::new(Image {
        width,
        height,
        pixels,
    });
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let bytes = image.pixels.len();
        if bytes <= 16 * 1024 * 1024 {
            while c.len() >= 16
                || c.iter().map(|e| e.image.pixels.len()).sum::<usize>() + bytes > 16 * 1024 * 1024
            {
                c.pop_front();
            }
            c.push_back(Entry {
                source,
                rect,
                sigma,
                mask,
                image: image.clone(),
            });
        }
    });
    Ok(image)
}

/// Three separable sliding boxes approximate a Gaussian, with work independent of radius.
fn boxes(pixels: &mut [u8], w: usize, h: usize, sigma: f32) {
    let ideal = (4.0 * sigma * sigma + 1.0).sqrt();
    let mut lower = ideal.floor() as usize;
    if lower.is_multiple_of(2) {
        lower = lower.saturating_sub(1);
    }
    lower = lower.max(1);
    let n = ((12.0 * sigma * sigma - 3.0 * (lower * lower) as f32 - 12.0 * lower as f32 - 9.0)
        / (-4.0 * lower as f32 - 4.0))
        .round()
        .clamp(0.0, 3.0) as usize;
    let mut tmp = vec![0; pixels.len()];
    for pass in 0..3 {
        let r = if pass < n { lower / 2 } else { lower / 2 + 1 };
        for (src, dst, lines, len, stride, step) in
            [(true, false, h, w, w * 4, 4), (false, true, w, h, 4, w * 4)]
        {
            let (input, output): (&[u8], &mut [u8]) = if src && !dst {
                (&*pixels, &mut tmp)
            } else {
                (&tmp, pixels)
            };
            for line in 0..lines {
                for ch in 0..4 {
                    let base = line * stride + ch;
                    let at = |i: isize| {
                        input[base + i.clamp(0, len as isize - 1) as usize * step] as u32
                    };
                    let mut sum = (0..=2 * r)
                        .map(|i| at(i as isize - r as isize))
                        .sum::<u32>();
                    for i in 0..len {
                        output[base + i * step] =
                            ((sum + (2 * r + 1) as u32 / 2) / (2 * r + 1) as u32) as u8;
                        sum = sum + at(i as isize + r as isize + 1) - at(i as isize - r as isize);
                    }
                }
            }
        }
    }
}

fn patch(
    text: &mut crate::Text,
    source: &[Op],
    bounds: Rect,
    damage: Rect,
    sigma: f32,
    mask: Option<(Rect, f32, f32)>,
) -> Result<Option<(Rect, Arc<Image>)>> {
    let output = bounds.intersect(damage);
    if output.is_empty() {
        return Ok(None);
    }
    let r = output.inflate(reach(sigma));
    let rect = Rect {
        left: r.left.floor(),
        top: r.top.floor(),
        right: r.right.ceil(),
        bottom: r.bottom.ceil(),
    };
    let source = source
        .iter()
        .filter(|op| matches!(op, Op::Clip { .. } | Op::Unclip) || op.bounds().intersects(&rect))
        .cloned()
        .collect();
    Ok(Some((rect, raster(text, source, rect, sigma, mask)?)))
}

pub(crate) fn prepare<'a>(
    text: &mut crate::Text,
    ops: &'a [Op],
    damage: Rect,
) -> Result<Cow<'a, [Op]>> {
    prepare_mode(text, ops, damage, false)
}
#[cfg(feature = "gpu")]
pub(crate) fn prepare_native<'a>(
    text: &mut crate::Text,
    ops: &'a [Op],
    damage: Rect,
) -> Result<Cow<'a, [Op]>> {
    prepare_mode(text, ops, damage, true)
}
fn prepare_mode<'a>(
    text: &mut crate::Text,
    ops: &'a [Op],
    damage: Rect,
    native: bool,
) -> Result<Cow<'a, [Op]>> {
    fn has(ops: &[Op]) -> bool {
        ops.iter().any(|o| match o {
            Op::Blurred { .. } | Op::BackdropBlur { .. } => true,
            Op::Transformed { ops, .. }
            | Op::Clipped { ops, .. }
            | Op::RoundedClipped { ops, .. } => has(ops),
            _ => false,
        })
    }
    if !has(ops) {
        return Ok(Cow::Borrowed(ops));
    }
    fn padding(ops: &[Op]) -> f32 {
        ops.iter()
            .map(|op| match op {
                Op::Blurred { sigma, ops } => reach(*sigma) + padding(ops),
                Op::BackdropBlur { sigma, .. } => reach(*sigma),
                Op::Transformed { ops, .. }
                | Op::Clipped { ops, .. }
                | Op::RoundedClipped { ops, .. } => padding(ops),
                _ => 0.0,
            })
            .sum()
    }
    Ok(Cow::Owned(walk(
        text,
        ops,
        &[],
        damage.inflate(padding(ops)),
        native,
    )?))
}
fn walk(
    text: &mut crate::Text,
    ops: &[Op],
    behind: &[Op],
    damage: Rect,
    native: bool,
) -> Result<Vec<Op>> {
    let mut out = Vec::new();
    for op in ops {
        match op {
            Op::Blurred { sigma, ops } if native => {
                out.push(Op::Blurred {
                    sigma: sigma.clamp(0.0, 64.0),
                    ops: walk(text, ops, &[], op.bounds(), native)?.into(),
                });
            }
            Op::Blurred { sigma, ops } => {
                let bounds = op.bounds();
                let source = walk(text, ops, &[], bounds, native)?;
                if let Some((rect, image)) = patch(text, &source, bounds, damage, *sigma, None)? {
                    out.push(Op::Image {
                        rect,
                        image,
                        opacity: 1.0,
                        source: None,
                    });
                }
            }
            Op::BackdropBlur {
                rect,
                radius,
                sigma,
                opacity,
            } => {
                let source: Vec<_> = behind.iter().chain(&out).cloned().collect();
                // Source-over is exact for an opaque backdrop. Preserve alpha with the
                // software replacement path when that cannot be established from the list.
                let input = rect.inflate(reach(*sigma));
                let opaque=source.iter().any(|op| matches!(op,Op::Fill{rect,radius,color} if *radius==0.0 && color.a==1.0 && rect.left<=input.left && rect.top<=input.top && rect.right>=input.right && rect.bottom>=input.bottom))
                    && source.iter().all(|op| !matches!(op,Op::Hole{..}|Op::Clipped{..}|Op::RoundedClipped{..}|Op::Transformed{..}));
                if native && *opacity == 1.0 && opaque {
                    out.push(Op::RoundedClipped {
                        rect: *rect,
                        radius: *radius,
                        ops: vec![Op::Blurred {
                            sigma: sigma.clamp(0.0, 64.0),
                            ops: source.into(),
                        }]
                        .into(),
                    });
                } else if let Some((area, image)) = patch(
                    text,
                    &source,
                    *rect,
                    damage,
                    *sigma,
                    Some((*rect, *radius, *opacity)),
                )? {
                    let output = rect.intersect(damage);
                    // The hole clears the target itself, outside any clip: inside a clip layer
                    // (vello) it would clear only the layer, leaving the old backdrop beneath.
                    out.push(Op::Hole { rect: output });
                    out.push(Op::Clipped {
                        rect: output,
                        ops: vec![Op::Image {
                            rect: area,
                            image,
                            opacity: 1.0,
                            source: None,
                        }]
                        .into(),
                    });
                }
            }
            Op::Transformed { transform, ops } => {
                if let Some(inverse) = transform.inverse() {
                    let prefix: Vec<_> = behind.iter().chain(&out).cloned().collect();
                    let prefix = vec![Op::Transformed {
                        transform: inverse,
                        ops: prefix.into(),
                    }];
                    out.push(Op::Transformed {
                        transform: *transform,
                        ops: walk(text, ops, &prefix, inverse.bounds(damage), native)?.into(),
                    });
                }
            }
            Op::Clipped { rect, ops } | Op::RoundedClipped { rect, ops, .. } => {
                let prefix: Vec<_> = behind.iter().chain(&out).cloned().collect();
                let inner = walk(text, ops, &prefix, damage.intersect(*rect), native)?.into();
                out.push(match op {
                    Op::Clipped { .. } => Op::Clipped {
                        rect: *rect,
                        ops: inner,
                    },
                    Op::RoundedClipped { radius, .. } => Op::RoundedClipped {
                        rect: *rect,
                        radius: *radius,
                        ops: inner,
                    },
                    _ => unreachable!(),
                });
            }
            _ => out.push(op.clone()),
        }
    }
    Ok(out)
}
