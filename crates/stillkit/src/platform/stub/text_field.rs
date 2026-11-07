use anyhow::{Result, bail};
#[derive(Debug, Clone, PartialEq)]
pub struct TextField;
impl TextField {
    pub fn revision(&self) -> u64 { unimplemented!("native text input") }
}
impl crate::platform::TextInput for TextField {
    fn text(&self) -> Result<String> { bail!("no native text input") }
    fn set_text(&self, _: &str) -> Result<()> { bail!("no native text input") }
    fn focus(&self) { unimplemented!("native IME focus") }
    fn blur(&self) { unimplemented!("native IME focus") }
    fn caret(&self) -> Option<crate::Rect> { unimplemented!("native IME caret") }
}
