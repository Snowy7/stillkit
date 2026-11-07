use anyhow::{Result, bail};
/// Ticker.
pub struct Ticker;
impl Ticker {
    /// Creates a new instance.
    pub fn new() -> Result<Self> {
        bail!("no native frame clock")
    }
    /// Schedule.
    pub fn schedule(&mut self, _: bool) {
        unimplemented!("native frame clock")
    }
    /// Armed.
    pub fn armed(&self) -> bool {
        unimplemented!("native frame clock")
    }
}
/// Display frame.
pub fn display_frame() -> std::time::Duration {
    unimplemented!("display refresh query")
}
