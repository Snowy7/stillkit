use super::*;
use stillkit::Image;
use stillkit::el::{PointerEvent, PointerPhase, image};

/// A colour as hue (degrees), saturation, value and alpha (0 to 1): the [`ColorPicker`]'s
/// model. Keep it in your state while the picker is open, so the hue survives greys and
/// blacks that RGB cannot remember.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsva {
    /// Hue in degrees, 0 to 360.
    pub hue: f32,
    /// Saturation, 0 to 1.
    pub saturation: f32,
    /// Value (brightness), 0 to 1.
    pub value: f32,
    /// Alpha, 0 to 1.
    pub alpha: f32,
}

impl Hsva {
    /// The model of `c` (a grey or black takes hue 0).
    pub fn from_rgba(c: Rgba) -> Self {
        let (r, g, b) = (c.r, c.g, c.b);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let hue = if delta <= f32::EPSILON {
            0.
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.) * 60.
        } else if max == g {
            ((b - r) / delta + 2.) * 60.
        } else {
            ((r - g) / delta + 4.) * 60.
        };
        Self {
            hue,
            saturation: if max <= 0. { 0. } else { delta / max },
            value: max,
            alpha: c.a,
        }
    }

    /// The colour.
    pub fn rgba(self) -> Rgba {
        let h = self.hue.rem_euclid(360.) / 60.;
        let chroma = self.value * self.saturation;
        let x = chroma * (1. - (h % 2. - 1.).abs());
        let (r, g, b) = match h as u8 {
            0 => (chroma, x, 0.),
            1 => (x, chroma, 0.),
            2 => (0., chroma, x),
            3 => (0., x, chroma),
            4 => (x, 0., chroma),
            _ => (chroma, 0., x),
        };
        let m = self.value - chroma;
        Rgba::new(r + m, g + m, b + m, self.alpha)
    }
}

type Pick<S> = Arc<dyn Fn(&mut S, Hsva, bool)>;

/// A controlled colour picker: a saturation and brightness square, a hue strip and an
/// optional alpha strip, each dragged to pick. `on_change(state, colour, done)` runs on every
/// move with `done` false (preview it) and once more on release with `done` true (commit
/// it). Rebuild from the colour you keep:
/// `ColorPicker::new(state.picking).alpha(true).on_change(|s, c, done| ..).build()`.
pub struct ColorPicker<S> {
    color: Hsva,
    alpha: bool,
    width: f32,
    change: Option<Pick<S>>,
}

/// Which part of the picker a drag moves.
#[derive(Clone, Copy)]
enum Part {
    Square,
    Hue,
    Alpha,
}

impl<S: 'static> ColorPicker<S> {
    pub fn new(color: Hsva) -> Self {
        Self {
            color,
            alpha: false,
            width: 220.,
            change: None,
        }
    }
    /// Show an alpha strip.
    pub fn alpha(mut self, alpha: bool) -> Self {
        self.alpha = alpha;
        self
    }
    /// The picker's width in design px (the square is three quarters as tall).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(80.);
        self
    }
    pub fn on_change(mut self, f: impl Fn(&mut S, Hsva, bool) + 'static) -> Self {
        self.change = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let (w, c) = (self.width, self.color);
        let h = (w * 0.75).round();
        let square = self
            .area(Part::Square, h)
            .child(image(square(c.hue)).w(w).h(h).radius(6.))
            .child(knob(c.rgba().with_alpha(1.)).absolute(c.saturation * w - 7., (1. - c.value) * h - 7.))
            .role(Role::ColorWell)
            .label(format!(
                "Saturation {:.0}%, brightness {:.0}%",
                c.saturation * 100.,
                c.value * 100.
            ));
        let hue = self
            .area(Part::Hue, 12.)
            .child(image(hue_strip()).w(w).h(12.).radius(6.))
            .child(knob(Hsva { saturation: 1., value: 1., alpha: 1., ..c }.rgba()).absolute(c.hue / 360. * w - 7., -1.))
            .role(Role::Slider)
            .label(format!("Hue {:.0}°", c.hue));
        let mut out = col().w(w).gap(10.).child(square).child(hue);
        if self.alpha {
            out = out.child(
                self.area(Part::Alpha, 12.)
                    .child(image(alpha_strip(c.rgba())).w(w).h(12.).radius(6.))
                    .child(knob(c.rgba().with_alpha(1.)).absolute(c.alpha * w - 7., -1.))
                    .role(Role::Slider)
                    .label(format!("Opacity {:.0}%", c.alpha * 100.)),
            );
        }
        out
    }

    /// A dragged area `h` tall: presses and moves pick from where the pointer is.
    fn area(&self, part: Part, h: f32) -> El<S> {
        let mut area = el().w(self.width).h(h);
        if let Some(f) = self.change.clone() {
            let color = self.color;
            area = area.on_pointer(move |s, e: &PointerEvent, _| {
                let done = match e.phase {
                    PointerPhase::Down | PointerPhase::Move => false,
                    PointerPhase::Up | PointerPhase::Cancel => true,
                    PointerPhase::Hover => return false,
                };
                f(s, pick(color, part, e), done);
                true
            });
        }
        area
    }
}

/// `color` with `part` set from where `e` is in its area (clamped while dragging past it).
fn pick(color: Hsva, part: Part, e: &PointerEvent) -> Hsva {
    let x = (e.local.x / e.size.0.max(1.)).clamp(0., 1.);
    let y = (e.local.y / e.size.1.max(1.)).clamp(0., 1.);
    match part {
        Part::Square => Hsva {
            saturation: x,
            value: 1. - y,
            ..color
        },
        Part::Hue => Hsva {
            hue: x * 359.999,
            ..color
        },
        Part::Alpha => Hsva { alpha: x, ..color },
    }
}

/// A round handle filled with `fill`, ringed white over a dark edge so it reads on any colour.
fn knob<S>(fill: Rgba) -> El<S> {
    el().size(14.)
        .radius(7.)
        .fill(fill)
        .border(2., Rgba::new(1., 1., 1., 1.))
        .box_shadow(stillkit::SoftShadow {
            blur: 2.,
            offset: (0., 1.),
            spread: 0.,
            color: Rgba::new(0., 0., 0., 0.45),
        })
}

/// A BGRA image `width` x `height` from a colour per pixel.
fn gradient(width: u32, height: u32, color: impl Fn(u32, u32) -> Rgba) -> Arc<Image> {
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let c = color(x, y);
            pixels.extend_from_slice(&[byte(c.b), byte(c.g), byte(c.r), 255]);
        }
    }
    Arc::new(Image {
        width,
        height,
        pixels,
    })
}

/// Saturation across, brightness down, at `hue`: small, the renderer smooths it up.
fn square(hue: f32) -> Arc<Image> {
    const N: u32 = 48;
    gradient(N, N, |x, y| {
        Hsva {
            hue,
            saturation: x as f32 / (N - 1) as f32,
            value: 1. - y as f32 / (N - 1) as f32,
            alpha: 1.,
        }
        .rgba()
    })
}

/// Every hue, left to right (made once).
fn hue_strip() -> Arc<Image> {
    static HUE: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    HUE.get_or_init(|| {
        gradient(360, 1, |x, _| {
            Hsva {
                hue: x as f32,
                saturation: 1.,
                value: 1.,
                alpha: 1.,
            }
            .rgba()
        })
    })
    .clone()
}

/// `color` from clear to opaque over a checkerboard, baked opaque.
fn alpha_strip(color: Rgba) -> Arc<Image> {
    const W: u32 = 128;
    gradient(W, 6, |x, y| {
        let check = if (x / 3 + y / 3) % 2 == 0 { 0.82 } else { 0.55 };
        let a = x as f32 / (W - 1) as f32;
        let mix = |c: f32| c * a + check * (1. - a);
        Rgba::new(mix(color.r), mix(color.g), mix(color.b), 1.)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsva_round_trips_rgba() {
        for c in [
            Rgba::new(0.8, 0.1, 0.1, 1.),
            Rgba::new(0.25, 0.28, 0.32, 0.5),
            Rgba::new(0., 0., 0., 1.),
            Rgba::new(1., 1., 1., 1.),
        ] {
            let back = Hsva::from_rgba(c).rgba();
            for (a, b) in [(c.r, back.r), (c.g, back.g), (c.b, back.b), (c.a, back.a)] {
                assert!((a - b).abs() < 1e-5, "{c:?} came back {back:?}");
            }
        }
    }
}
