use super::*;

/// A surface's optics. `None` tint inherits the palette's surface in either scheme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    /// Glass.
    Glass {
        /// Clarity.
        clarity: f32,
        /// Blur.
        blur: f32,
        /// Refraction.
        refraction: f32,
        /// Highlight.
        highlight: f32,
        /// Tint.
        tint: Option<Rgba>,
        /// Tint alpha.
        tint_alpha: Option<f32>,
        /// Lensed.
        lensed: bool,
    },
    /// Fill.
    Fill {
        /// Tint.
        tint: Option<Rgba>,
    },
}

/// Generic material sliders. Lens controls are honored only by a lensed material.
pub type MaterialOverrides = Knobs;

impl Material {
    /// Liquid.
    pub fn liquid() -> Self {
        Self::Glass {
            clarity: 0.55,
            blur: 12.0,
            refraction: 0.6,
            highlight: 0.75,
            tint: None,
            tint_alpha: None,
            lensed: true,
        }
    }
    /// One ui.
    pub fn one_ui() -> Self {
        Self::Glass {
            clarity: 0.35,
            blur: 24.0,
            refraction: 0.0,
            highlight: 0.0,
            tint: None,
            tint_alpha: None,
            lensed: false,
        }
    }
    /// Fluent.
    pub fn fluent() -> Self {
        Self::Glass {
            clarity: 0.12,
            blur: 30.0,
            refraction: 0.0,
            highlight: 0.0,
            tint: None,
            tint_alpha: None,
            lensed: false,
        }
    }
    /// Solid.
    pub fn solid() -> Self {
        Self::Fill { tint: None }
    }
    /// Mono.
    pub fn mono() -> Self {
        Self::Fill { tint: None }
    }
    /// Tint.
    pub fn tint(mut self, value: Rgba) -> Self {
        match &mut self {
            Self::Glass {
                tint, tint_alpha, ..
            } => {
                *tint = Some(value);
                *tint_alpha = None;
            }
            Self::Fill { tint } => *tint = Some(value),
        }
        self
    }
    /// Clarity.
    pub fn clarity(self, value: f32) -> Self {
        self.overrides(&MaterialOverrides {
            clarity: Some(value),
            ..Default::default()
        })
    }
    /// Blur.
    pub fn blur(self, value: f32) -> Self {
        self.overrides(&MaterialOverrides {
            blur: Some(value),
            ..Default::default()
        })
    }
    /// Refraction.
    pub fn refraction(self, value: f32) -> Self {
        self.overrides(&MaterialOverrides {
            refraction: Some(value),
            ..Default::default()
        })
    }
    /// Highlight.
    pub fn highlight(self, value: f32) -> Self {
        self.overrides(&MaterialOverrides {
            highlight: Some(value),
            ..Default::default()
        })
    }
    /// Overrides.
    pub fn overrides(self, knobs: &MaterialOverrides) -> Self {
        match self {
            Self::Fill { .. } => self,
            Self::Glass {
                clarity,
                blur,
                refraction,
                highlight,
                tint,
                tint_alpha,
                lensed,
            } => {
                let unit = |v: f32| {
                    if v.is_finite() {
                        v.clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                };
                let clarity = knobs.clarity.map_or(clarity, unit);
                Self::Glass {
                    clarity,
                    blur: knobs
                        .blur
                        .map_or(blur, |v| if v.is_finite() { v.max(0.0) } else { 0.0 }),
                    refraction: if lensed {
                        knobs.refraction.map_or(refraction, unit)
                    } else {
                        0.0
                    },
                    highlight: if lensed {
                        knobs.highlight.map_or(highlight, unit)
                    } else {
                        0.0
                    },
                    tint,
                    tint_alpha: knobs.clarity.map(|_| 1.0 - clarity).or(tint_alpha),
                    lensed,
                }
            }
        }
    }
    pub(super) fn resolve(self, surface: Rgba) -> (Rgba, Option<GlassMaterial>) {
        match self {
            Self::Fill { tint } => (tint.unwrap_or(surface), None),
            Self::Glass {
                clarity,
                blur,
                refraction,
                highlight,
                tint,
                tint_alpha,
                lensed,
            } => {
                let tint = tint.unwrap_or(surface);
                let tint = tint_alpha.map_or(tint, |a| tint.with_alpha(a));
                (
                    tint,
                    Some(GlassMaterial {
                        clarity,
                        blur,
                        refraction: if lensed { refraction } else { 0.0 },
                        highlight: if lensed { highlight } else { 0.0 },
                        tint,
                    }),
                )
            }
        }
    }
}

/// Shape values; cards are derived from surface minus padding, floored at `card_min`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// Surface.
    pub surface: f32,
    /// Card min.
    pub card_min: f32,
    /// Control.
    pub control: Option<f32>,
    /// Small.
    pub small: f32,
}
impl Shape {
    /// Liquid.
    pub fn liquid() -> Self {
        Self {
            surface: 24.0,
            card_min: 10.0,
            control: None,
            small: 6.0,
        }
    }
    /// One ui.
    pub fn one_ui() -> Self {
        Self {
            surface: 28.0,
            card_min: 16.0,
            control: None,
            small: 6.0,
        }
    }
    /// Solid.
    pub fn solid() -> Self {
        Self {
            surface: 14.0,
            card_min: 6.0,
            control: Some(8.0),
            small: 6.0,
        }
    }
    /// Fluent.
    pub fn fluent() -> Self {
        Self {
            surface: 8.0,
            card_min: 4.0,
            control: Some(4.0),
            small: 4.0,
        }
    }
    /// Mono.
    pub fn mono() -> Self {
        Self {
            surface: 16.0,
            card_min: 8.0,
            control: Some(10.0),
            small: 6.0,
        }
    }
}
/// Which radius to override. Card sets its minimum; its concentric radius is still derived.
#[derive(Debug, Clone, Copy)]
pub enum Radius {
    /// Surface.
    Surface,
    /// Card.
    Card,
    /// Control.
    Control,
    /// Small.
    Small,
}
/// Spacing.
pub type Spacing = Metrics;
/// Motion.
pub type Motion = MotionTokens;
/// Icon drawing source and stroke weight.
#[derive(Debug, Clone, PartialEq)]
pub struct Icons {
    /// Set.
    pub set: IconSet,
    /// Weight.
    pub weight: f32,
}
impl Icons {
    /// Liquid.
    pub fn liquid() -> Self {
        Self {
            set: IconSet::Vector,
            weight: 1.75,
        }
    }
    /// One ui.
    pub fn one_ui() -> Self {
        Self {
            weight: 2.0,
            ..Self::liquid()
        }
    }
    /// Solid.
    pub fn solid() -> Self {
        Self::liquid()
    }
    /// Fluent.
    pub fn fluent() -> Self {
        Self {
            set: IconSet::Font(crate::icons::SYSTEM_FAMILY.into()),
            weight: 2.0,
        }
    }
    /// Mono.
    pub fn mono() -> Self {
        Self::liquid()
    }
}
impl Metrics {
    /// Liquid.
    pub fn liquid() -> Self {
        Self {
            pad: 12.0,
            gap: 12.0,
            item: 6.0,
            row: 46.0,
            row_single: 36.0,
            control: 30.0,
            control_small: 24.0,
            icon: 16.0,
            icon_small: 13.0,
            icon_large: 20.0,
            hit: 28.0,
            scrollbar: 5.0,
            scrollbar_inset: 3.0,
        }
    }
    /// One ui.
    pub fn one_ui() -> Self {
        Self {
            row: 52.0,
            row_single: 40.0,
            ..Self::liquid()
        }
    }
    /// Solid.
    pub fn solid() -> Self {
        Self {
            pad: 10.0,
            ..Self::liquid()
        }
    }
    /// Fluent.
    pub fn fluent() -> Self {
        Self {
            pad: 8.0,
            gap: 8.0,
            control: 32.0,
            ..Self::liquid()
        }
    }
    /// Mono.
    pub fn mono() -> Self {
        Self::solid()
    }
}
/// Per-look elevation. Strengths multiply the shadow colour's alpha (0.18 light, 0.45 dark), so
/// a strength of 0.8 is about 0.14 in light schemes.
impl Shadows {
    /// macOS: a faint contact edge under a wide, soft lift (a menu's ambient shadow is about
    /// `0 12px 32px` at 0.15, as Tahoe's menus are).
    pub fn liquid() -> Self {
        let key = ShadowLayer::new(3.0, 1.0, 0.0, 0.4);
        Self {
            card: [ShadowLayer::new(2.0, 1.0, 0.0, 0.3), ShadowLayer::new(10.0, 3.0, 0.0, 0.35)],
            raised: [key, ShadowLayer::new(18.0, 6.0, 0.0, 0.55)],
            menu: [key, ShadowLayer::new(32.0, 12.0, 0.0, 0.85)],
            popover: [key, ShadowLayer::new(36.0, 14.0, 0.0, 0.85)],
            dialog: [ShadowLayer::new(6.0, 2.0, 0.0, 0.45), ShadowLayer::new(60.0, 24.0, 0.0, 1.0)],
            toast: [key, ShadowLayer::new(28.0, 10.0, 0.0, 0.75)],
        }
    }
    /// One UI: lighter and more diffuse than Liquid, barely offset.
    pub fn one_ui() -> Self {
        let key = ShadowLayer::new(4.0, 1.0, 0.0, 0.25);
        Self {
            card: [ShadowLayer::new(2.0, 0.5, 0.0, 0.2), ShadowLayer::new(14.0, 2.0, 0.0, 0.3)],
            raised: [key, ShadowLayer::new(24.0, 4.0, 0.0, 0.45)],
            menu: [key, ShadowLayer::new(40.0, 8.0, 0.0, 0.7)],
            popover: [key, ShadowLayer::new(44.0, 10.0, 0.0, 0.7)],
            dialog: [ShadowLayer::new(6.0, 2.0, 0.0, 0.3), ShadowLayer::new(64.0, 16.0, 0.0, 0.85)],
            toast: [key, ShadowLayer::new(36.0, 8.0, 0.0, 0.65)],
        }
    }
    /// Windows 11 (Fluent 2's shadow ramp: a 0.12 outline blur and a 0.14 drop, `blur = 2y`):
    /// cards shadow4, raised shadow8, menus and flyouts shadow16, popovers and toasts
    /// shadow28 (teaching tips), dialogs shadow64.
    pub fn fluent() -> Self {
        let ramp = |key: f32, depth: f32| {
            [ShadowLayer::new(key, 0.0, 0.0, 0.67), ShadowLayer::new(depth, depth / 2.0, 0.0, 0.78)]
        };
        Self {
            card: ramp(2.0, 4.0),
            raised: ramp(2.0, 8.0),
            menu: ramp(2.0, 16.0),
            popover: ramp(8.0, 28.0),
            dialog: ramp(8.0, 64.0),
            toast: ramp(8.0, 28.0),
        }
    }
    /// Opaque surfaces with a hairline: a crisp contact edge and a short, quiet lift.
    pub fn solid() -> Self {
        let key = ShadowLayer::new(2.0, 1.0, 0.0, 0.4);
        Self {
            card: [ShadowLayer::new(2.0, 1.0, 0.0, 0.35), ShadowLayer::NONE],
            raised: [key, ShadowLayer::new(8.0, 3.0, 0.0, 0.35)],
            menu: [key, ShadowLayer::new(16.0, 6.0, 0.0, 0.5)],
            popover: [key, ShadowLayer::new(18.0, 7.0, 0.0, 0.5)],
            dialog: [ShadowLayer::new(3.0, 1.0, 0.0, 0.45), ShadowLayer::new(32.0, 12.0, 0.0, 0.6)],
            toast: [key, ShadowLayer::new(14.0, 5.0, 0.0, 0.45)],
        }
    }
    /// Monochrome: Solid's, a little quieter.
    pub fn mono() -> Self {
        let mut s = Self::solid();
        for level in [&mut s.card, &mut s.raised, &mut s.menu, &mut s.popover, &mut s.dialog, &mut s.toast] {
            for layer in level {
                layer.strength *= 0.85;
            }
        }
        s
    }
}

impl MotionTokens {
    /// Liquid.
    pub fn liquid() -> Self {
        Self {
            interactive: Spring::INTERACTIVE,
            open: Spring::apple(0.3, 0.12),
            layout: Spring::SNAPPY,
            scroll: Spring::apple(0.22, 0.0),
            scrollbar_idle: 0.9,
            scrollbar_fade: 0.35,
        }
    }
    /// One ui.
    pub fn one_ui() -> Self {
        Self::liquid()
    }
    /// Solid.
    pub fn solid() -> Self {
        Self::liquid()
    }
    /// Fluent.
    pub fn fluent() -> Self {
        Self::liquid()
    }
    /// Mono.
    pub fn mono() -> Self {
        Self::liquid()
    }
}
pub(super) struct PalettePreset {
    pub(super) surface: [(u32, f32); 2],
    pub(super) card: [(u32, f32); 2],
    pub(super) steps: [(f32, f32, f32); 2],
    pub(super) monochrome: bool,
}
const STEPS: [(f32, f32, f32); 2] = [(0.78, 0.60, 0.34), (0.72, 0.50, 0.30)];
const LIQUID: PalettePreset = PalettePreset {
    surface: [(0xF5F5F7, 0.80), (0x1C1C1E, 0.82)],
    card: [(0xFFFFFF, 0.55), (0xFFFFFF, 0.10)],
    steps: STEPS,
    monochrome: false,
};
const ONE_UI: PalettePreset = PalettePreset {
    surface: [(0xF4F4F6, 0.92), (0x17171A, 0.92)],
    card: [(0xFFFFFF, 0.90), (0xFFFFFF, 0.09)],
    steps: STEPS,
    monochrome: false,
};
const SOLID: PalettePreset = PalettePreset {
    surface: [(0xF2F2F7, 1.0), (0x1C1C1E, 1.0)],
    card: [(0xFFFFFF, 1.0), (0x2C2C2E, 1.0)],
    steps: STEPS,
    monochrome: false,
};
const FLUENT: PalettePreset = PalettePreset {
    surface: [(0xF3F3F3, 0.90), (0x202020, 0.90)],
    card: [(0xFFFFFF, 0.70), (0xFFFFFF, 0.06)],
    steps: [(0.80, 0.62, 0.42), (0.79, 0.55, 0.36)],
    monochrome: false,
};
const MONO: PalettePreset = PalettePreset {
    surface: [(0xF7F7F8, 0.78), (0x141416, 0.78)],
    monochrome: true,
    ..LIQUID
};
macro_rules! presets {
    ($($name:ident => ($style:ident, $palette:ident, $ramp:ident, $border:expr)),*) => {
        impl TypeRamp { $(#[doc = "Fn."] pub fn $name() -> Self { $ramp() })* }
        impl Palette { $(#[doc = "Fn."] pub fn $name(scheme: Scheme, accent: Rgba) -> Self { palette(&$palette, scheme == Scheme::Dark, accent) })* }
        impl Theme { $(#[doc = "Fn."] pub fn $name(scheme: Scheme, accent: Rgba) -> Self {
            let mut t = Self::compose(Style::$style, scheme, [Palette::$name(Scheme::Light, accent), Palette::$name(Scheme::Dark, accent)],
                Material::$name(), Shape::$name(), TypeRamp::$name(), Spacing::$name(), Motion::$name(), Icons::$name(), Shadows::$name());
            t.border = if $border { Some(t.color.separator) } else { None };
            t
        })* }
    }
}
presets!(liquid => (Liquid, LIQUID, ramp_liquid, false), one_ui => (OneUi, ONE_UI, ramp_one_ui, false), solid => (Solid, SOLID, ramp_liquid, true), fluent => (Fluent, FLUENT, ramp_fluent, false), mono => (Mono, MONO, ramp_liquid, false));

/// A contrast requirement which cannot be satisfied by the chosen surface and ink.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// Role.
    pub role: &'static str,
    /// Ratio.
    pub ratio: f32,
    /// Required.
    pub required: f32,
}

impl Theme {
    #[allow(clippy::too_many_arguments)]
    fn compose(
        style: Style,
        scheme: Scheme,
        palettes: [Palette; 2],
        material: Material,
        shape: Shape,
        font: TypeRamp,
        metrics: Metrics,
        motion: Motion,
        icons: Icons,
        shadows: Shadows,
    ) -> Self {
        let color = palettes[usize::from(scheme == Scheme::Dark)];
        Self {
            style,
            scheme,
            color,
            font,
            metrics,
            radius: Radii {
                surface: shape.surface,
                card: Radii::concentric(shape.surface, metrics.pad, shape.card_min),
                control: shape.control,
                small: shape.small,
            },
            elevation: Elevation {
                spread: 16.0,
                drop: 4.0,
                color: color.shadow,
            },
            shadows,
            motion,
            icons: icons.set,
            icon_weight: icons.weight,
            glass: material.resolve(color.surface).1,
            border: None,
            material,
            focus_ring: FocusRing::default(),
            card_min: shape.card_min,
            mixed: false,
            palettes,
            monochrome: color.monochrome,
        }
    }
    /// Material.
    pub fn material(mut self, material: Material) -> Self {
        self.material = material;
        self.recompute();
        self
    }
    /// Shape.
    pub fn shape(mut self, shape: Shape) -> Self {
        self.radius.surface = shape.surface;
        self.radius.control = shape.control;
        self.radius.small = shape.small;
        self.card_min = shape.card_min;
        self.recompute();
        self
    }
    /// Radius.
    pub fn radius(mut self, radius: Radius, value: f32) -> Self {
        match radius {
            Radius::Surface => self.radius.surface = value,
            Radius::Card => self.card_min = value,
            Radius::Control => self.radius.control = Some(value),
            Radius::Small => self.radius.small = value,
        }
        self.recompute();
        self
    }
    /// Type ramp.
    pub fn type_ramp(mut self, ramp: TypeRamp) -> Self {
        self.font = ramp;
        self.recompute();
        self
    }
    /// Spacing.
    pub fn spacing(mut self, spacing: Spacing) -> Self {
        self.metrics = spacing;
        self.recompute();
        self
    }
    /// Another look's elevation levels (`Shadows::fluent()` under a Liquid theme, say).
    pub fn shadows(mut self, shadows: Shadows) -> Self {
        self.shadows = shadows;
        self.recompute();
        self
    }
    /// Motion.
    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self.recompute();
        self
    }
    /// Icons.
    pub fn icons(mut self, icons: Icons) -> Self {
        self.icons = icons.set;
        self.icon_weight = icons.weight;
        self.recompute();
        self
    }
    /// Set both scheme palettes so adaptive scheme changes retain the mix.
    pub fn palettes(mut self, light: Palette, dark: Palette) -> Self {
        self.palettes = [light, dark];
        self.recompute();
        self
    }
    /// Palette.
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palettes[usize::from(self.dark())] = palette;
        self.recompute();
        self
    }
    /// Focus ring.
    pub fn focus_ring(mut self, ring: FocusRing) -> Self {
        self.focus_ring = ring;
        self.recompute();
        self
    }
    /// Border.
    pub fn border(mut self, border: Option<Rgba>) -> Self {
        self.border = border;
        self.recompute();
        self
    }
    fn backgrounds(&self) -> Vec<Rgba> {
        let under = if self.dark() {
            hex(0, 1.0)
        } else {
            hex(0xFFFFFF, 1.0)
        };
        let surface = over(self.color.surface, under);
        let mut backgrounds = vec![surface, over(self.color.card, surface)];
        // Very clear glass can expose either extreme of an unknown backdrop.
        if self.glass.is_some_and(|g| g.tint.a < 0.5) {
            let other = over(
                self.color.surface,
                if self.dark() {
                    hex(0xFFFFFF, 1.0)
                } else {
                    hex(0, 1.0)
                },
            );
            backgrounds.extend([other, over(self.color.card, other)]);
        }
        backgrounds
    }
    pub(super) fn recompute(&mut self) {
        self.mixed = true;
        self.radius.card = Radii::concentric(self.radius.surface, self.metrics.pad, self.card_min);
        self.color = self.palettes[usize::from(self.dark())];
        self.monochrome = self.color.monochrome;
        (self.color.surface, self.glass) = self.material.resolve(self.color.surface);
        let backgrounds = self.backgrounds();
        let solve = |mut c: Rgba, floor: f32| {
            for _ in 0..100 {
                if backgrounds
                    .iter()
                    .all(|bg| contrast(over(c, *bg), *bg) >= floor)
                {
                    break;
                }
                if c.a < 1.0 {
                    c.a = (c.a + 0.01).min(1.0);
                } else {
                    c = mix(
                        c,
                        if luminance(backgrounds[0]) < 0.18 {
                            hex(0xFFFFFF, 1.0)
                        } else {
                            hex(0, 1.0)
                        },
                        0.1,
                    );
                }
            }
            c
        };
        self.color.label = solve(self.color.label, 7.0);
        self.color.secondary = solve(self.color.secondary, 4.5);
        self.color.tertiary = solve(self.color.tertiary, 3.0);
        self.color.accent = solve(self.color.accent, 3.0);
        self.color.on_accent = if contrast(hex(0xFFFFFF, 1.0), self.color.accent) >= 3.0 {
            hex(0xFFFFFF, 1.0)
        } else {
            hex(0x111113, 1.0)
        };
        self.color.accent_wash =
            self.color
                .accent
                .with_alpha(if self.dark() { 0.24 } else { 0.16 });
        self.elevation.color = self.color.shadow;
    }
    /// Check.
    pub fn check(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (role, ink, required) in [
            ("label", self.color.label, 7.0),
            ("secondary", self.color.secondary, 4.5),
            ("tertiary", self.color.tertiary, 3.0),
            ("accent", self.color.accent, 3.0),
        ] {
            for (index, bg) in self.backgrounds().into_iter().enumerate() {
                if role == "accent" && index % 2 == 1 {
                    continue;
                }
                let ratio = contrast(over(ink, bg), bg);
                if ratio < required {
                    problems.push(Problem {
                        role,
                        ratio,
                        required,
                    });
                }
            }
        }
        let ratio = contrast(self.color.on_accent, self.color.accent);
        if ratio < 3.0 {
            problems.push(Problem {
                role: "on_accent",
                ratio,
                required: 3.0,
            });
        }
        problems
    }
}

/// Focus ring values for hosts and components (design pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    /// Width in device pixels.
    pub width: f32,
    /// Gap.
    pub gap: f32,
    /// Opacity.
    pub opacity: f32,
}
impl Default for FocusRing {
    fn default() -> Self {
        Self {
            width: 2.0,
            gap: 2.0,
            opacity: 0.9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_pass_and_parts_survive_scheme_changes() {
        for style in Style::ALL {
            for scheme in [Scheme::Dark, Scheme::Light] {
                let t = Theme::new(style, scheme, SYSTEM_BLUE);
                assert!(
                    t.check().is_empty(),
                    "{style:?} {scheme:?}: {:?}",
                    t.check()
                );
            }
        }
        let t = Theme::liquid(Scheme::Dark, SYSTEM_BLUE)
            .shape(Shape::one_ui())
            .type_ramp(TypeRamp::one_ui())
            .spacing(Spacing {
                pad: 8.0,
                ..Spacing::liquid()
            })
            .radius(Radius::Control, 10.0);
        assert_eq!(t.radius.card, 20.0);
        let light = t.with_scheme(Scheme::Light);
        assert_eq!(light.radius, t.radius);
        assert_eq!(light.font, t.font);
        assert!(light.check().is_empty(), "{:?}", light.check());
    }
    #[test]
    fn shadow_levels_follow_the_scheme_survive_mixing_and_turn_off_with_the_colour() {
        let t = Theme::liquid(Scheme::Light, SYSTEM_BLUE).shadows(Shadows::fluent());
        let [key, ambient] = t.shadow(ShadowLevel::Menu);
        // Fluent's shadow16 (0 8px 16px at 0.14) in a light scheme.
        assert_eq!((ambient.blur, ambient.offset), (16.0, (0.0, 8.0)));
        assert!((ambient.color.a - 0.14).abs() < 0.005, "{}", ambient.color.a);
        assert!(key.color.a > 0.0);
        let mixed = t.clone().shape(Shape::one_ui()).with_scheme(Scheme::Dark);
        assert_eq!(mixed.shadows, Shadows::fluent());
        assert!(mixed.shadow(ShadowLevel::Menu)[1].color.a > ambient.color.a, "dark is stronger");
        // Each look rises from cards to dialogs.
        for style in Style::ALL {
            let t = Theme::new(style, Scheme::Light, SYSTEM_BLUE);
            let reach = |l| t.shadow(l).map(|s| s.blur + s.offset.1).into_iter().fold(0.0, f32::max);
            assert!(reach(ShadowLevel::Card) < reach(ShadowLevel::Menu), "{style:?}");
            assert!(reach(ShadowLevel::Menu) < reach(ShadowLevel::Dialog), "{style:?}");
        }
        let mut off = t;
        off.elevation.color = Rgba::TRANSPARENT;
        assert!(off.shadow(ShadowLevel::Dialog).iter().all(|s| s.color.a == 0.0));
    }
    #[test]
    fn lens_is_owned_by_material_and_contrast_is_resolved_after_tint_changes() {
        let t =
            Theme::one_ui(Scheme::Dark, SYSTEM_BLUE).material(Material::liquid().refraction(0.3));
        assert_eq!(t.glass.unwrap().refraction, 0.3);
        let clear = t.clone().material(Material::liquid().clarity(1.0));
        assert!(
            !clear.check().is_empty(),
            "clear glass cannot guarantee contrast on every backdrop"
        );
        assert_eq!(
            Material::one_ui()
                .refraction(0.9)
                .resolve(t.color.surface)
                .1
                .unwrap()
                .refraction,
            0.0
        );
        let t = t.material(Material::solid().tint(hex(0xFFFFFF, 1.0)));
        assert!(t.check().is_empty(), "{:?}", t.check());
    }
    #[cfg(feature = "serde")]
    #[test]
    fn look_files_mix_and_reject_unknown_properties() {
        let t = Theme::from_toml(include_str!("../../looks/sunset.toml")).unwrap();
        assert_eq!(t.glass.unwrap().refraction, 0.3);
        assert_eq!(t.radius.control, Some(10.0));
        let json = Theme::from_json(r#"{"base":"one_ui","material":{"base":"liquid","refraction":0.3},"shape":{"control":10.0}}"#).unwrap();
        assert_eq!(t, json);
        assert!(Theme::from_json(r#"{"materail":{}}"#).is_err());
    }
}
