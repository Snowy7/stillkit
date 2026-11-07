use std::{cell::RefCell, rc::Rc};
type WindowId = usize;
use crate::glass::{GlassMaterial, GlassStyle, Shape};
/// Where glass requests go: the engine, through the shell's overlay client. Every call is for
/// `client`, the surface's window.
pub trait Backdrop {
    /// Whether glass can show now (the engine runs). Views draw a fill of their own when not.
    fn available(&self) -> bool;
    /// Creates the glass under `client`; `false` when it was refused.
    fn open(&self, client: WindowId, shape: Shape, radius: f32, material: &GlassMaterial) -> bool;
    /// Opens with optional optics; old backdrops can continue implementing only `open`.
    fn open_styled(
        &self,
        client: WindowId,
        shape: Shape,
        radius: f32,
        material: &GlassMaterial,
        style: &GlassStyle,
    ) -> bool {
        let opened = self.open(client, shape, radius, material);
        if opened {
            self.style(client, style);
        }
        opened
    }
    /// Style.
    fn style(&self, _client: WindowId, _style: &GlassStyle) {}
    /// Place.
    fn place(&self, client: WindowId, shape: Shape, radius: f32);
    /// Material.
    fn material(&self, client: WindowId, material: &GlassMaterial);
    /// Close.
    fn close(&self, client: WindowId);
}

thread_local! {
    static BACKDROP: RefCell<Option<Rc<dyn Backdrop>>> = const { RefCell::new(None) };

}

/// Sends this thread's glass requests to `backdrop` from now on.
pub fn install(backdrop: Rc<dyn Backdrop>) {
    BACKDROP.with_borrow_mut(|b| *b = Some(backdrop));
}

/// The installed backdrop, if any.
pub(crate) fn installed() -> Option<Rc<dyn Backdrop>> {
    BACKDROP.with_borrow(Clone::clone)
}

/// Where this thread's glass goes: only an installed backdrop (no system one on this platform).
pub(crate) fn backdrop() -> Option<Rc<dyn Backdrop>> {
    installed()
}

/// Whether a built-in system backdrop can show `material`: there is none on this platform.
pub(crate) fn system_serves(_material: &GlassMaterial) -> bool {
    false
}
