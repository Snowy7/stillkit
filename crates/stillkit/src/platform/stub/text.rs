pub use crate::text_style::{TextStyle, VARIABLE};
/// Text.
pub struct Text;
impl Text {
    /// Measures the text in device pixels.
    pub fn measure(&mut self, _: &str, _: &TextStyle) -> (f32, f32) {
        unimplemented!("text shaping")
    }
    /// Releases cached native text layouts.
    pub fn clear_cache(&mut self) {
        unimplemented!("text cache")
    }
    /// Sets capacity.
    pub fn set_capacity(&mut self, _: usize) {
        unimplemented!("text cache")
    }
    /// Has family.
    pub fn has_family(&self, _: &str) -> bool {
        unimplemented!("font lookup")
    }
    /// Families.
    pub fn families(&self) -> Vec<String> {
        unimplemented!("font enumeration")
    }
    /// Fit.
    pub fn fit(&mut self, _: &str, _: &TextStyle, _: f32) -> std::sync::Arc<str> {
        unimplemented!("text fitting")
    }
    /// Lines.
    pub fn lines(&mut self, _: &str, _: &TextStyle, _: f32, _: usize) -> Vec<std::sync::Arc<str>> {
        unimplemented!("text wrapping")
    }
}
impl super::super::TextSystem for Text {
    type Format = ();
    type Layout = ();
    fn format(&mut self, _: &TextStyle) -> Option<()> {
        unimplemented!("text format")
    }
    fn layout(&mut self, _: &str, _: &TextStyle) -> Option<&()> {
        unimplemented!("text layout")
    }
    fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
        self.measure(text, style)
    }
    fn clear_cache(&mut self) {
        self.clear_cache()
    }
}
