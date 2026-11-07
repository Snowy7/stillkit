//! Version 1 look documents: a base preset plus optional part presets and properties.
use super::*;
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum Preset {
    #[default]
    Liquid,
    #[serde(alias = "oneui")]
    OneUi,
    Solid,
    Fluent,
    Mono,
}
impl Preset {
    fn theme(self, scheme: Scheme, accent: Rgba) -> Theme {
        match self {
            Self::Liquid => Theme::liquid(scheme, accent),
            Self::OneUi => Theme::one_ui(scheme, accent),
            Self::Solid => Theme::solid(scheme, accent),
            Self::Fluent => Theme::fluent(scheme, accent),
            Self::Mono => Theme::mono(scheme, accent),
        }
    }
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Look {
    base: Preset,
    scheme: Option<String>,
    accent: Option<[f32; 4]>,
    material: Option<MaterialData>,
    shape: Option<ShapeData>,
    type_ramp: Option<TypeData>,
    spacing: Option<SpacingData>,
    motion: Option<MotionData>,
    icons: Option<IconsData>,
    palette: Option<PaletteData>,
    border: Option<BorderData>,
    focus_ring: Option<FocusData>,
    shadows: Option<ShadowsData>,
}
/// Another look's elevation levels.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ShadowsData {
    base: Option<Preset>,
}
fn rgba([r, g, b, a]: [f32; 4]) -> anyhow::Result<Rgba> {
    anyhow::ensure!(
        [r, g, b, a]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
        "color channels must be in 0..=1"
    );
    Ok(Rgba::new(r, g, b, a))
}
fn preset(base: Option<Preset>, theme: &Theme) -> Option<Theme> {
    base.map(|p| p.theme(theme.scheme, theme.color.accent))
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct MaterialData {
    base: Option<Preset>,
    clarity: Option<f32>,
    blur: Option<f32>,
    refraction: Option<f32>,
    highlight: Option<f32>,
    tint: Option<[f32; 4]>,
    lensed: Option<bool>,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ShapeData {
    base: Option<Preset>,
    surface: Option<f32>,
    card_min: Option<f32>,
    control: Option<f32>,
    capsule: Option<bool>,
    small: Option<f32>,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct TypeData {
    base: Option<Preset>,
    family: Option<String>,
    roles: Option<[FontData; 10]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontData {
    size: f32,
    weight: u16,
    tracking: f32,
    line: f32,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct IconsData {
    base: Option<Preset>,
    family: Option<String>,
    vector: Option<bool>,
    weight: Option<f32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpringData {
    stiffness: f32,
    damping: f32,
    mass: f32,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct SpacingData {
    base: Option<Preset>,
    pad: Option<f32>,
    gap: Option<f32>,
    item: Option<f32>,
    row: Option<f32>,
    row_single: Option<f32>,
    control: Option<f32>,
    control_small: Option<f32>,
    icon: Option<f32>,
    icon_small: Option<f32>,
    icon_large: Option<f32>,
    hit: Option<f32>,
    scrollbar: Option<f32>,
    scrollbar_inset: Option<f32>,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct PaletteData {
    base: Option<Preset>,
    monochrome: Option<bool>,
    label: Option<[f32; 4]>,
    secondary: Option<[f32; 4]>,
    tertiary: Option<[f32; 4]>,
    quaternary: Option<[f32; 4]>,
    fill: Option<[f32; 4]>,
    card: Option<[f32; 4]>,
    well: Option<[f32; 4]>,
    separator: Option<[f32; 4]>,
    accent: Option<[f32; 4]>,
    on_accent: Option<[f32; 4]>,
    accent_wash: Option<[f32; 4]>,
    destructive: Option<[f32; 4]>,
    success: Option<[f32; 4]>,
    warning: Option<[f32; 4]>,
    surface: Option<[f32; 4]>,
    elevated: Option<[f32; 4]>,
    hover: Option<[f32; 4]>,
    pressed: Option<[f32; 4]>,
    knob: Option<[f32; 4]>,
    scrollbar: Option<[f32; 4]>,
    shadow: Option<[f32; 4]>,
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct MotionData {
    base: Option<Preset>,
    interactive: Option<SpringData>,
    open: Option<SpringData>,
    layout: Option<SpringData>,
    scroll: Option<SpringData>,
    scrollbar_idle: Option<f32>,
    scrollbar_fade: Option<f32>,
}
impl Look {
    fn resolve(self) -> anyhow::Result<Theme> {
        let scheme = match self.scheme.as_deref().unwrap_or("dark") {
            "dark" => Scheme::Dark,
            "light" => Scheme::Light,
            _ => anyhow::bail!("scheme must be light or dark"),
        };
        let mut t = self.base.theme(
            scheme,
            self.accent.map(rgba).transpose()?.unwrap_or(SYSTEM_BLUE),
        );
        if let Some(p) = self.material {
            let mut m = preset(p.base, &t).map_or(t.material, |t| t.material);
            if let (Some(value), Material::Glass { lensed, .. }) = (p.lensed, &mut m) {
                *lensed = value;
            }
            m = m.overrides(&MaterialOverrides {
                clarity: p.clarity,
                blur: p.blur,
                refraction: p.refraction,
                highlight: p.highlight,
            });
            if let Some(tint) = p.tint {
                m = m.tint(rgba(tint)?);
            }
            t = t.material(m);
        }
        if let Some(p) = self.shape {
            let source = preset(p.base, &t).unwrap_or_else(|| t.clone());
            let mut shape = Shape {
                surface: source.radius.surface,
                card_min: source.card_min,
                control: source.radius.control,
                small: source.radius.small,
            };
            if let Some(v) = p.surface {
                shape.surface = v;
            }
            if let Some(v) = p.card_min {
                shape.card_min = v;
            }
            if let Some(v) = p.control {
                shape.control = Some(v);
            }
            if p.capsule == Some(true) {
                shape.control = None;
            }
            if let Some(v) = p.small {
                shape.small = v;
            }
            t = t.shape(shape);
        }
        if let Some(p) = self.type_ramp {
            let mut value = preset(p.base, &t).map_or_else(|| t.font.clone(), |t| t.font);
            if let Some(family) = p.family {
                value.family = family.into();
            }
            if let Some(roles) = p.roles {
                value.roles = roles.map(|f| Font {
                    size: f.size,
                    weight: f.weight,
                    tracking: f.tracking,
                    line: f.line,
                });
            }
            t = t.type_ramp(value);
        }
        if let Some(p) = self.spacing {
            let mut value = preset(p.base, &t).map_or(t.metrics, |t| t.metrics);
            if let Some(v) = p.pad {
                value.pad = v;
            }
            if let Some(v) = p.gap {
                value.gap = v;
            }
            if let Some(v) = p.item {
                value.item = v;
            }
            if let Some(v) = p.row {
                value.row = v;
            }
            if let Some(v) = p.row_single {
                value.row_single = v;
            }
            if let Some(v) = p.control {
                value.control = v;
            }
            if let Some(v) = p.control_small {
                value.control_small = v;
            }
            if let Some(v) = p.icon {
                value.icon = v;
            }
            if let Some(v) = p.icon_small {
                value.icon_small = v;
            }
            if let Some(v) = p.icon_large {
                value.icon_large = v;
            }
            if let Some(v) = p.hit {
                value.hit = v;
            }
            if let Some(v) = p.scrollbar {
                value.scrollbar = v;
            }
            if let Some(v) = p.scrollbar_inset {
                value.scrollbar_inset = v;
            }
            t = t.spacing(value);
        }
        if let Some(p) = self.palette {
            let mut value = preset(p.base, &t).map_or(t.color, |t| t.color);
            if let Some(v) = p.label {
                value.label = rgba(v)?;
            }
            if let Some(v) = p.secondary {
                value.secondary = rgba(v)?;
            }
            if let Some(v) = p.tertiary {
                value.tertiary = rgba(v)?;
            }
            if let Some(v) = p.quaternary {
                value.quaternary = rgba(v)?;
            }
            if let Some(v) = p.fill {
                value.fill = rgba(v)?;
            }
            if let Some(v) = p.card {
                value.card = rgba(v)?;
            }
            if let Some(v) = p.well {
                value.well = rgba(v)?;
            }
            if let Some(v) = p.separator {
                value.separator = rgba(v)?;
            }
            if let Some(v) = p.accent {
                value.accent = rgba(v)?;
            }
            if let Some(v) = p.on_accent {
                value.on_accent = rgba(v)?;
            }
            if let Some(v) = p.accent_wash {
                value.accent_wash = rgba(v)?;
            }
            if let Some(v) = p.destructive {
                value.destructive = rgba(v)?;
            }
            if let Some(v) = p.success {
                value.success = rgba(v)?;
            }
            if let Some(v) = p.warning {
                value.warning = rgba(v)?;
            }
            if let Some(v) = p.surface {
                value.surface = rgba(v)?;
            }
            if let Some(v) = p.elevated {
                value.elevated = rgba(v)?;
            }
            if let Some(v) = p.hover {
                value.hover = rgba(v)?;
            }
            if let Some(v) = p.pressed {
                value.pressed = rgba(v)?;
            }
            if let Some(v) = p.knob {
                value.knob = rgba(v)?;
            }
            if let Some(v) = p.scrollbar {
                value.scrollbar = rgba(v)?;
            }
            if let Some(v) = p.shadow {
                value.shadow = rgba(v)?;
            }
            if let Some(v) = p.monochrome {
                value.monochrome = v;
            }
            t = t.palette(value);
        }
        if let Some(p) = self.motion {
            let mut value = preset(p.base, &t).map_or(t.motion, |t| t.motion);
            if let Some(v) = p.interactive {
                value.interactive = Spring {
                    stiffness: v.stiffness,
                    damping: v.damping,
                    mass: v.mass,
                };
            }
            if let Some(v) = p.open {
                value.open = Spring {
                    stiffness: v.stiffness,
                    damping: v.damping,
                    mass: v.mass,
                };
            }
            if let Some(v) = p.layout {
                value.layout = Spring {
                    stiffness: v.stiffness,
                    damping: v.damping,
                    mass: v.mass,
                };
            }
            if let Some(v) = p.scroll {
                value.scroll = Spring {
                    stiffness: v.stiffness,
                    damping: v.damping,
                    mass: v.mass,
                };
            }
            if let Some(v) = p.scrollbar_idle {
                value.scrollbar_idle = v;
            }
            if let Some(v) = p.scrollbar_fade {
                value.scrollbar_fade = v;
            }
            t = t.motion(value);
        }
        if let Some(p) = self.icons {
            let source = preset(p.base, &t).unwrap_or_else(|| t.clone());
            let mut value = Icons {
                set: source.icons,
                weight: source.icon_weight,
            };
            if let Some(family) = p.family {
                value.set = IconSet::Font(family.into());
            }
            if p.vector == Some(true) {
                value.set = IconSet::Vector;
            }
            if let Some(v) = p.weight {
                value.weight = v;
            }
            t = t.icons(value);
        }
        if let Some(border) = self.border {
            let border = match border {
                BorderData::Color(c) => Some(rgba(c)?),
                BorderData::Enabled(true) => Some(t.color.separator),
                BorderData::Enabled(false) => None,
            };
            t = t.border(border);
        }
        if let Some(p) = self.focus_ring {
            let ring = t.focus_ring;
            t = t.focus_ring(FocusRing {
                width: p.width.unwrap_or(ring.width),
                gap: p.gap.unwrap_or(ring.gap),
                opacity: p.opacity.unwrap_or(ring.opacity),
            });
        }
        if let Some(p) = self.shadows {
            let shadows = preset(p.base, &t).map_or(t.shadows, |t| t.shadows);
            t = t.shadows(shadows);
        }
        Ok(t)
    }
}
impl Theme {
    /// Load a version 1 look document (see docs/ui.md, Looks).
    pub fn from_toml(source: &str) -> anyhow::Result<Self> {
        toml::from_str::<Look>(source)?.resolve()
    }
    /// JSON uses the same schema as [`Theme::from_toml`].
    pub fn from_json(source: &str) -> anyhow::Result<Self> {
        serde_json::from_str::<Look>(source)?.resolve()
    }
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct FocusData {
    width: Option<f32>,
    gap: Option<f32>,
    opacity: Option<f32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BorderData {
    Color([f32; 4]),
    Enabled(bool),
}
