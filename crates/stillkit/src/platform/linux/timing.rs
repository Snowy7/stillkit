//! Live animation is driven by Surface frame callbacks, never a periodic timer.
#[derive(Default)]
pub struct Ticker {
    armed: bool,
}
impl Ticker {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self::default())
    }
    pub fn schedule(&mut self, moving: bool) {
        self.armed = moving;
    }
    pub fn armed(&self) -> bool {
        self.armed
    }
}
pub fn display_frame() -> std::time::Duration {
    std::time::Duration::from_nanos(16_666_667)
}
