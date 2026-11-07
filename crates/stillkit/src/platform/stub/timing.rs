use anyhow::{Result, bail};
pub struct Ticker;
impl Ticker {
    pub fn new() -> Result<Self> { bail!("no native frame clock") }
    pub fn schedule(&mut self, _: bool) { unimplemented!("native frame clock") }
    pub fn armed(&self) -> bool { unimplemented!("native frame clock") }
}
pub fn display_frame() -> std::time::Duration { unimplemented!("display refresh query") }
