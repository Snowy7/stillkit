//! Portable bitmap and rasterization options.
/// How DirectWrite rasterizes text: always grayscale (layered windows have alpha, so no
/// ClearType), with fixed parameters rather than the machine's ClearType tuning, so golden
/// images and every machine look the same. [`TextQuality::default`] is what every surface
/// uses; the gallery's text lab (`docs/media/native/kit-text.png`) compares the others.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextQuality {
    pub gamma: f32,
    /// Grayscale enhanced contrast: thickens the coverage of light strokes.
    pub contrast: f32,
    pub mode: TextMode,
    /// Hint vertical stems and heights to whole pixels.
    pub grid_fit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    /// Subpixel positions, antialiased in both directions (DirectWrite's default above 12 px).
    NaturalSymmetric,
    /// Subpixel positions, antialiased horizontally only.
    Natural,
    /// Whole-pixel advances and full hinting, as GDI.
    GdiClassic,
}

impl Default for TextQuality {
    /// Chosen in the text lab (docs/ui.md, "Text"): Windows' grayscale defaults (gamma 1.8,
    /// grayscale contrast 1.0) with vertical hinting. The old parameters set contrast 0.5
    /// through the ClearType-only field, which drew light text thin and grey on glass.
    fn default() -> Self {
        Self {
            gamma: 1.8,
            contrast: 1.0,
            mode: TextMode::NaturalSymmetric,
            grid_fit: true,
        }
    }
}

/// A premultiplied BGRA image, top-down, `width * 4` bytes per row.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Image {
    /// The `[b, g, r, a]` of a pixel.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[at],
            self.pixels[at + 1],
            self.pixels[at + 2],
            self.pixels[at + 3],
        ]
    }
}
