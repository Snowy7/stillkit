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
    fn style(&self, _client: WindowId, _style: &GlassStyle) {}
    fn place(&self, client: WindowId, shape: Shape, radius: f32);
    fn material(&self, client: WindowId, material: &GlassMaterial);
    fn close(&self, client: WindowId);
}

thread_local! {
    static BACKDROP: RefCell<Option<Rc<dyn Backdrop>>> = const { RefCell::new(None) };

}

/// Sends this thread's glass requests to `backdrop` from now on.
pub fn install(backdrop: Rc<dyn Backdrop>) {
    BACKDROP.with_borrow_mut(|b| *b = Some(backdrop));
}

pub(crate) fn backdrop() -> Option<Rc<dyn Backdrop>> {
    BACKDROP.with_borrow(Clone::clone)
}
