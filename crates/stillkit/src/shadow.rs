//! Soft shadows ([`crate::Op::SoftShadow`]): a Gaussian-blurred rounded rectangle, computed
//! analytically once per shape into an alpha mask, then drawn as a 9-slice.
//!
//! The blur is separable, so the shadow of a shape cut into horizontal bands (each band a run
//! `[left, right] × [top, bottom]`) is exactly the sum over bands of
//! `X(x) · Y(y)`, where `X` and `Y` are differences of the Gaussian CDF (erf) at the band's
//! edges. A rounded rectangle is one full-width band plus thin bands down its two corner caps,
//! each as wide as the arc at its middle, so only the cap discretisation is approximate (bands
//! at most a pixel and a quarter-σ tall: far below one level of 255 once blurred).
//!
//! Away from its corners the shadow of a large box is constant along each axis, so the mask is
//! made for the smallest box that still has a constant middle (corners `radius + 3σ` deep) and
//! stretched: drawing a moving or resizing menu reuses it. A box too small for that along an
//! axis gets a mask of its exact size along that axis.

/// The Gaussian's σ for a CSS blur radius (`blur = 2σ`).
pub(crate) fn sigma(blur: f32) -> f32 {
    blur.max(0.0) / 2.0
}

/// Beyond this many σ outside the shape a shadow is zero (the tail is 0.13 % of its colour,
/// under half a level).
pub(crate) const REACH: f32 = 3.0;

/// The shape a shadow blurs: `rect` grown by `spread` (corners too) and moved by `offset`.
pub(crate) fn shape(
    rect: crate::Rect,
    radius: f32,
    offset: (f32, f32),
    spread: f32,
) -> (crate::Rect, f32) {
    (
        rect.inflate(spread).offset(offset.0, offset.1),
        (radius + spread).max(0.0),
    )
}

#[cfg(any(windows, test))]
pub(crate) use mask::*;

#[cfg(any(windows, test))]
mod mask {
    use crate::Rect;

    /// Identifies a mask: radius and σ in quarter pixels, and each axis' exact length or 0
    /// when it is stretched.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) struct Key {
        radius: u32,
        sigma: u32,
        width: u32,
        height: u32,
    }

    /// One run of mask pixels drawn onto device pixels (stretched when the lengths differ).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) struct Slice {
        pub src: u32,
        pub src_len: u32,
        pub dst: f32,
        pub dst_len: f32,
    }

    /// How one shadow is drawn: its mask and the slices along each axis (one when exact, three
    /// when stretched). Every pairing of an x slice with a y slice is one draw.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct Plan {
        pub key: Key,
        pub x: Vec<Slice>,
        pub y: Vec<Slice>,
    }

    impl Plan {
        /// Each (mask source, device destination) rectangle pair.
        pub fn pieces(&self) -> impl Iterator<Item = (Rect, Rect)> + '_ {
            self.y.iter().flat_map(move |y| {
                self.x.iter().map(move |x| {
                    (
                        Rect::new(x.src as f32, y.src as f32, x.src_len as f32, y.src_len as f32),
                        Rect::new(x.dst, y.dst, x.dst_len, y.dst_len),
                    )
                })
            })
        }
    }

    /// A shadow's coverage, row by row (0–255; the colour's alpha scales it when drawn).
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct Mask {
        pub width: u32,
        pub height: u32,
        pub alpha: Vec<u8>,
    }

    impl Key {
        fn radius(self) -> f32 {
            self.radius as f32 / 4.0
        }
        fn sigma(self) -> f32 {
            self.sigma as f32 / 4.0
        }
        /// Outside the shape: where the mask starts (its outer pixels' centres 3σ out).
        fn margin(self) -> u32 {
            (super::REACH * self.sigma() + 0.5).ceil() as u32
        }
        /// Inside the shape: past this depth from an edge the coverage no longer varies along it.
        fn depth(self) -> u32 {
            (self.radius() + super::REACH * self.sigma()).ceil() as u32
        }
        /// The shape's length in the mask along an axis of key length `exact`. A stretched axis
        /// has three constant pixels in the middle: the centre one is stretched, and its
        /// neighbours keep a filtered stretch exact.
        fn shape_len(self, exact: u32) -> u32 {
            if exact == 0 { 2 * self.depth() + 3 } else { exact }
        }
        pub fn size(self) -> (u32, u32) {
            let m = 2 * self.margin();
            (self.shape_len(self.width) + m, self.shape_len(self.height) + m)
        }
        /// Bytes the mask holds.
        pub fn bytes(self) -> usize {
            let (w, h) = self.size();
            w as usize * h as usize
        }
    }

    /// How to draw the blurred `shape` (device pixels, corner `radius`) with Gaussian `sigma`
    /// (> 0), or `None` when the shape is empty. Edges snap to whole pixels, so slices meet
    /// without seams; half a pixel is invisible under a blur.
    pub(crate) fn plan(shape: Rect, radius: f32, sigma: f32) -> Option<Plan> {
        let (left, top) = (shape.left.round(), shape.top.round());
        let (w, h) = (
            (shape.right.round() - left).max(0.0) as u32,
            (shape.bottom.round() - top).max(0.0) as u32,
        );
        if w == 0 || h == 0 || !sigma.is_finite() || !radius.is_finite() {
            return None;
        }
        let quarter = |v: f32| (v * 4.0).max(0.0).min(u16::MAX as f32);
        let radius = quarter(radius.min(w as f32 / 2.0).min(h as f32 / 2.0)).floor() as u32;
        let sigma = (quarter(sigma).round() as u32).max(2);
        let probe = Key { radius, sigma, width: 0, height: 0 };
        let (margin, depth) = (probe.margin(), probe.depth());
        let stretch = |len: u32| len > 2 * depth;
        let key = Key {
            width: if stretch(w) { 0 } else { w },
            height: if stretch(h) { 0 } else { h },
            ..probe
        };
        let axis = |start: f32, len: u32, exact: u32| -> Vec<Slice> {
            let (m, start) = (margin as f32, start - margin as f32);
            if exact != 0 {
                let all = len + 2 * margin;
                return vec![Slice { src: 0, src_len: all, dst: start, dst_len: all as f32 }];
            }
            let corner = margin + depth;
            let middle = (len - 2 * depth) as f32;
            vec![
                Slice { src: 0, src_len: corner, dst: start, dst_len: corner as f32 },
                Slice { src: corner + 1, src_len: 1, dst: start + corner as f32, dst_len: middle },
                Slice {
                    src: corner + 3,
                    src_len: corner,
                    dst: start + m + len as f32 - depth as f32,
                    dst_len: corner as f32,
                },
            ]
        };
        Some(Plan {
            key,
            x: axis(left, w, key.width),
            y: axis(top, h, key.height),
        })
    }

    /// Renders `key`'s mask: the shape centred in it, blurred, sampled at pixel centres.
    pub(crate) fn mask(key: Key) -> Mask {
        let (width, height) = key.size();
        let (r, sigma, m) = (key.radius(), key.sigma(), key.margin() as f32);
        let (sw, sh) = (key.shape_len(key.width) as f32, key.shape_len(key.height) as f32);
        let (left, top, right, bottom) = (m, m, m + sw, m + sh);
        // Bands (top, bottom, left, right): the straight middle, then thin bands down each cap.
        let mut bands = vec![(top + r, bottom - r, left, right)];
        if r > 0.0 {
            let n = (r / (sigma / 4.0).clamp(0.25, 1.0)).ceil().max(1.0) as usize;
            let step = r / n as f32;
            for i in 0..n {
                let (a, b) = (i as f32 * step, (i + 1) as f32 * step);
                let d = r - (a + b) / 2.0;
                let inset = r - (r * r - d * d).max(0.0).sqrt();
                bands.push((top + a, top + b, left + inset, right - inset));
                bands.push((bottom - b, bottom - a, left + inset, right - inset));
            }
        }
        // Coverage is symmetric about the centre: compute the top-left quadrant, then mirror.
        let (qw, qh) = (width.div_ceil(2) as usize, height.div_ceil(2) as usize);
        let cdf = |t: f32| 0.5 * (1.0 + erf(t * std::f32::consts::FRAC_1_SQRT_2 / sigma));
        let along = |n: usize, lo: f32, hi: f32| -> Vec<f32> {
            (0..n)
                .map(|i| {
                    let c = i as f32 + 0.5;
                    cdf(c - lo) - cdf(c - hi)
                })
                .collect()
        };
        let mut sum = vec![0f32; qw * qh];
        for &(t, b, l, rr) in &bands {
            if b <= t || rr <= l {
                continue;
            }
            let xs = along(qw, l, rr);
            for (y, wy) in along(qh, t, b).into_iter().enumerate() {
                if wy < 1e-6 {
                    continue;
                }
                for (s, wx) in sum[y * qw..(y + 1) * qw].iter_mut().zip(&xs) {
                    *s += wy * wx;
                }
            }
        }
        let (w, h) = (width as usize, height as usize);
        let mut alpha = vec![0u8; w * h];
        for y in 0..h {
            let qy = y.min(h - 1 - y);
            for x in 0..w {
                let v = sum[qy * qw + x.min(w - 1 - x)];
                alpha[y * w + x] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        Mask { width, height, alpha }
    }

    /// erf, Abramowitz and Stegun 7.1.26 (error under 1.5e-7).
    fn erf(x: f32) -> f32 {
        let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
        let poly = t
            * (0.254_829_6
                + t * (-0.284_496_74 + t * (1.421_413_7 + t * (-1.453_152_1 + t * 1.061_405_4))));
        let e = 1.0 - poly * (-x * x).exp();
        if x < 0.0 { -e } else { e }
    }

    thread_local! {
        /// The masks of this UI thread, shared by its surfaces (a menu opening on a new surface
        /// reuses the last one's).
        pub(crate) static MASKS: std::cell::RefCell<Cache<std::sync::Arc<Mask>>> =
            std::cell::RefCell::new(Cache::new(Cache::<()>::BUDGET));
    }

    /// `key`'s mask from this thread's cache, rendered on first use.
    pub(crate) fn shared(key: Key) -> std::sync::Arc<Mask> {
        MASKS.with_borrow_mut(|cache| {
            cache.get(key, |key| Some(std::sync::Arc::new(mask(key)))).cloned()
        })
        .expect("a mask is always made")
    }

    /// A bounded cache of per-key values (made on first use), as the text cache is bounded:
    /// past `budget` bytes of masks the least recently used go first.
    pub(crate) struct Cache<T> {
        entries: Vec<(Key, T)>,
        bytes: usize,
        budget: usize,
    }

    impl<T> Cache<T> {
        /// 1 MB of masks: a 2× menu's is about 110 KB, a 1× one 30 KB.
        pub const BUDGET: usize = 1 << 20;

        pub fn new(budget: usize) -> Self {
            Self { entries: Vec::new(), bytes: 0, budget }
        }

        /// The value for `key`, made by `make` when missing (`None` from `make` is not kept).
        pub fn get(&mut self, key: Key, make: impl FnOnce(Key) -> Option<T>) -> Option<&T> {
            if let Some(i) = self.entries.iter().position(|(k, _)| *k == key) {
                // Most recently used last.
                let hit = self.entries.remove(i);
                self.entries.push(hit);
            } else {
                let value = make(key)?;
                self.bytes += key.bytes();
                self.entries.push((key, value));
                while self.bytes > self.budget && self.entries.len() > 1 {
                    let (old, _) = self.entries.remove(0);
                    self.bytes -= old.bytes();
                }
            }
            self.entries.last().map(|(_, v)| v)
        }

        #[cfg(test)]
        pub fn len(&self) -> usize {
            self.entries.len()
        }

        #[cfg(test)]
        pub fn bytes(&self) -> usize {
            self.bytes
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Draws `plan` with its mask onto a `w`×`h` coverage canvas (nearest sampling of the
        /// stretched middles, which are constant).
        fn assemble(plan: &Plan, w: usize, h: usize) -> Vec<u8> {
            let m = mask(plan.key);
            let mut out = vec![0u8; w * h];
            for (src, dst) in plan.pieces() {
                for y in dst.top as i32..dst.bottom as i32 {
                    for x in dst.left as i32..dst.right as i32 {
                        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                            continue;
                        }
                        let sx = src.left + ((x as f32 - dst.left) * src.width() / dst.width());
                        let sy = src.top + ((y as f32 - dst.top) * src.height() / dst.height());
                        out[y as usize * w + x as usize] =
                            m.alpha[sy as usize * m.width as usize + sx as usize];
                    }
                }
            }
            out
        }

        /// The mask of exactly this box (no stretching), placed on the same canvas.
        fn direct(shape: Rect, radius: f32, sigma: f32, w: usize, h: usize) -> Vec<u8> {
            let plan = plan(shape, radius, sigma).unwrap();
            let key = Key {
                width: shape.width() as u32,
                height: shape.height() as u32,
                ..plan.key
            };
            let exact = Plan {
                key,
                x: vec![Slice {
                    src: 0,
                    src_len: key.size().0,
                    dst: shape.left - key.margin() as f32,
                    dst_len: key.size().0 as f32,
                }],
                y: vec![Slice {
                    src: 0,
                    src_len: key.size().1,
                    dst: shape.top - key.margin() as f32,
                    dst_len: key.size().1 as f32,
                }],
            };
            assemble(&exact, w, h)
        }

        #[test]
        fn the_shadow_is_symmetric_and_falls_off_monotonically_from_its_edge() {
            let plan = plan(Rect::new(40.0, 40.0, 300.0, 400.0), 12.0, 8.0).unwrap();
            let m = mask(plan.key);
            let (w, h) = (m.width as usize, m.height as usize);
            let at = |x: usize, y: usize| m.alpha[y * w + x];
            for y in 0..h {
                for x in 0..w {
                    assert_eq!(at(x, y), at(w - 1 - x, y));
                    assert_eq!(at(x, y), at(x, h - 1 - y));
                }
            }
            let mid = h / 2;
            let (margin, centre) = (plan.key.margin() as usize, w / 2);
            // Rising from the outside to the middle of the box, along a row and down a diagonal.
            for x in 1..=centre {
                assert!(at(x, mid) >= at(x - 1, mid), "row at {x}");
            }
            for d in 1..=w.min(h) / 2 {
                assert!(at(d, d) >= at(d - 1, d - 1), "diagonal at {d}");
            }
            // Half strength on a straight edge (between the pixels either side of it), full
            // well inside it.
            let edge = at(margin - 1, mid) as i32 + at(margin, mid) as i32;
            assert!((edge - 255).abs() <= 2, "edge {edge}");
            assert_eq!(at(centre, mid), 255);
        }

        #[test]
        fn a_large_box_peaks_at_full_colour_and_is_zero_beyond_its_reach() {
            for (radius, sigma) in [(0.0, 4.0), (10.0, 6.0), (24.0, 12.0), (6.0, 0.5)] {
                let shape = Rect::new(0.0, 0.0, 400.0, 300.0);
                let m = mask(plan(shape, radius, sigma).unwrap().key);
                let peak = *m.alpha.iter().max().unwrap();
                assert!(peak >= 254, "{radius} {sigma}: peak {peak}");
                // The mask ends 3σ outside the shape, and its border is already zero.
                let w = m.width as usize;
                assert!(m.alpha[..w].iter().all(|&a| a == 0), "{radius} {sigma}: top row");
                assert!(
                    (0..m.height as usize).all(|y| m.alpha[y * w] == 0),
                    "{radius} {sigma}: left column"
                );
            }
        }

        #[test]
        fn the_nine_slices_reassemble_the_direct_render() {
            for (shape, radius, sigma) in [
                (Rect::new(30.0, 25.0, 300.0, 400.0), 12.0, 8.0),
                (Rect::new(60.0, 60.0, 240.0, 30.0), 6.0, 10.0),
                (Rect::new(20.0, 20.0, 150.0, 120.0), 30.0, 3.0),
            ] {
                let (w, h) = (shape.right as usize + 60, shape.bottom as usize + 60);
                let plan = plan(shape, radius, sigma).unwrap();
                let sliced = assemble(&plan, w, h);
                let exact = direct(shape, radius, sigma, w, h);
                let worst = sliced.iter().zip(&exact).map(|(a, b)| a.abs_diff(*b)).max();
                assert!(worst <= Some(1), "{shape:?}: {worst:?}");
            }
        }

        #[test]
        fn small_axes_use_their_exact_length_and_large_ones_stretch() {
            let tall = plan(Rect::new(0.0, 0.0, 400.0, 28.0), 6.0, 8.0).unwrap();
            assert_eq!((tall.key.width, tall.key.height), (0, 28));
            assert_eq!((tall.x.len(), tall.y.len()), (3, 1));
            // A moving or resizing box shares one mask.
            let moved = plan(Rect::new(37.0, 90.0, 520.0, 28.0), 6.0, 8.0).unwrap();
            assert_eq!(moved.key, tall.key);
            assert!(plan(Rect::new(0.0, 0.0, 0.0, 10.0), 4.0, 4.0).is_none());
        }

        #[test]
        fn the_mask_cache_stays_within_its_budget_and_keeps_recent_masks() {
            let mut cache = Cache::new(200_000);
            let mut made = 0;
            for size in 0..40 {
                let key = plan(Rect::new(0.0, 0.0, 4.0 + size as f32, 400.0), 4.0, 6.0)
                    .unwrap()
                    .key;
                cache.get(key, |_| {
                    made += 1;
                    Some(())
                });
                assert!(cache.bytes() <= 200_000 && cache.len() >= 1);
            }
            assert_eq!(made, 40);
            assert!(cache.len() < 40, "old masks were dropped");
            // The newest is still there: no new mask is made for it.
            let last = plan(Rect::new(0.0, 0.0, 43.0, 400.0), 4.0, 6.0).unwrap().key;
            cache.get(last, |_| panic!("cached"));
        }
    }
}
