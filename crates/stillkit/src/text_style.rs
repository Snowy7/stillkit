use std::sync::Arc;
/// How a run of text looks. `size` is in device pixels when it reaches the renderer; in
/// [`crate::Tokens`] it is in design pixels.
///
/// The family [`VARIABLE`] ("Segoe UI Variable") is drawn through its variable axes: any
/// `weight` (500 medium included) and an optical size that follows the design size, as
/// Windows 11 itself draws it. Other families use their named faces.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub family: Arc<str>,
    pub size: f32,
    /// 100..=900, 400 regular, 600 semibold.
    pub weight: u16,
    /// Extra space after every character, device px (negative tightens display sizes).
    pub tracking: f32,
    /// The optical size in points for variable fonts; 0 takes it from `size`.
    pub optical: f32,
}

/// Segoe UI Variable's typographic family: its `wght` and `opsz` axes are used directly.
pub const VARIABLE: &str = "Segoe UI Variable";

impl TextStyle {
    pub fn new(family: &str, size: f32) -> Self {
        Self {
            family: family.into(),
            size,
            weight: 400,
            tracking: 0.0,
            optical: 0.0,
        }
    }

    /// The same style at `scale` times the size (the optical size stays: it is physical).
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            size: self.size * scale,
            tracking: self.tracking * scale,
            optical: if self.optical > 0.0 {
                self.optical
            } else {
                self.size * 0.75
            },
            ..self.clone()
        }
    }

}
