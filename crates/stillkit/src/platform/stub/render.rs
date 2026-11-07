pub use crate::bitmap::{Image, TextMode, TextQuality};
use anyhow::{Result, bail};
/// Ui.
pub struct Ui {
    /// Text content.
    pub text: super::Text,
}
impl Ui {
    /// Creates a new instance.
    pub fn new() -> Result<Self> {
        bail!("no native backend for this platform")
    }
    /// Isolated.
    pub fn isolated() -> Result<Self> {
        Self::new()
    }
    /// Sets text quality.
    pub fn set_text_quality(&mut self, _: TextQuality) -> Result<()> {
        bail!("no native backend")
    }
    /// Decode image.
    pub fn decode_image(&self, _: &[u8], _: (u32, u32)) -> Result<Image> {
        bail!("no image decoder")
    }
}
impl Image {
    /// Load.
    pub fn load(_: &Ui, _: &std::path::Path) -> Result<Self> {
        bail!("no image decoder")
    }
}
/// Render.
pub fn render(_: &mut Ui, _: &crate::DisplayList, _: u32, _: u32) -> Result<Image> {
    bail!("no CPU renderer for this platform")
}
/// Canvas.
pub struct Canvas;
impl crate::platform::Renderer for Canvas {
    type Context = Ui;
    fn new(_: &Ui, _: u32, _: u32) -> Result<Self> {
        bail!("no CPU renderer")
    }
    fn draw(&self, _: &mut super::Text, _: &crate::DisplayList, _: crate::PixelRect) -> Result<()> {
        bail!("no CPU renderer")
    }
    fn read(&self, _: crate::PixelRect, _: impl FnMut(u32, &[u8])) -> Result<()> {
        bail!("no CPU renderer")
    }
}
