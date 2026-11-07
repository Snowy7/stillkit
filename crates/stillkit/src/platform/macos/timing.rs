//! Live animation runs on `Surface::animation_frames`, paced by the event wait's timeout at
//! the display's refresh interval; there is never a periodic timer while nothing moves.
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

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

/// One refresh of the main screen (ProMotion screens report 120 Hz), 60 Hz when unknown.
pub fn display_frame() -> std::time::Duration {
    let fps = MainThreadMarker::new()
        .and_then(NSScreen::mainScreen)
        .map_or(60, |s| s.maximumFramesPerSecond())
        .clamp(24, 240);
    std::time::Duration::from_nanos(1_000_000_000 / fps as u64)
}
