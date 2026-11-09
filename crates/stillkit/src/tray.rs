//! Opt-in system tray icons. Create and use on the UI thread; events wake `host::wait`.
use crate::platform::current::services;
use anyhow::{Result, ensure};
/// A click on the tray icon, in physical screen coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A primary click.
    Left(i32, i32),
    /// A secondary click; open a stillkit `Menu` here.
    Right(i32, i32),
    /// A primary double click (Windows).
    Double(i32, i32),
}
/// Straight RGBA pixels for a system tray icon.
#[derive(Clone, Debug)]
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
pub struct Icon {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}
impl Icon {
    /// Validates dimensions and the byte count; icons may be at most 256 pixels per side.
    pub fn rgba(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= 256 && height <= 256,
            "tray dimensions must be 1..=256"
        );
        ensure!(
            pixels.len() == width as usize * height as usize * 4,
            "tray pixel count differs from dimensions"
        );
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}
/// Owns a tray icon; dropping it removes it. Linux requires a StatusNotifier host
/// (GNOME needs an extension). macOS is currently an unsupported NSStatusItem stub.
pub struct Tray(services::Tray);
impl Tray {
    /// Creates an icon on this UI thread, without showing or activating any window.
    pub fn new(id: &str, tooltip: &str, icon: Icon) -> Result<Self> {
        services::Tray::new(id, tooltip, icon).map(Self)
    }
    /// Takes the next click. Call after dispatching native events or `host::wait`.
    pub fn next_event(&mut self) -> Option<Event> {
        self.0.next_event()
    }
    /// Changes the tooltip.
    pub fn set_tooltip(&mut self, text: &str) -> Result<()> {
        self.0.set_tooltip(text)
    }
    /// Opens stillkit's own menu at a click's screen position. Route its surface events
    /// to `Menu::handle` as usual; creating an icon alone never creates menu surfaces.
    pub fn open_menu<C: Clone>(
        &self,
        event: Event,
        menu: &mut crate::Menu<C>,
        ui: &mut crate::Ui,
        items: Vec<crate::Entry<C>>,
        tokens: &crate::Tokens,
    ) -> Result<()> {
        let (x, y) = match event {
            Event::Left(x, y) | Event::Right(x, y) | Event::Double(x, y) => (x, y),
        };
        menu.open(ui, items, (x, y), crate::screen_at(x, y), tokens)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_pixel_dimensions_before_platform_work() {
        assert!(Icon::rgba(0, 1, vec![]).is_err());
        assert!(Icon::rgba(u32::MAX, u32::MAX, vec![]).is_err());
        assert!(Icon::rgba(2, 2, vec![0; 15]).is_err());
        assert!(Icon::rgba(2, 2, vec![0; 16]).is_ok());
    }
}
