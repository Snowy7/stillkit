use crate::glass::{GlassMaterial, GlassStyle, Shape};
use std::{cell::RefCell, rc::Rc};
use windows::Win32::Foundation::HWND;
/// Where glass requests go: the engine, through the shell's overlay client, or the built-in
/// system backdrop ([`super::system_backdrop`]). Every call is for `client`, the surface's window.
pub trait Backdrop {
    /// Whether glass can show now (the engine runs). Views draw a fill of their own when not.
    fn available(&self) -> bool;
    /// Native frost is ready with the content; it needs no engine capture delay.
    fn synchronous(&self) -> bool { false }
    /// Atomically shows or hides a content/frost pair. False leaves showing to the surface.
    fn visibility(&self, _client: HWND, _shown: bool) -> bool { false }
    /// Match the opacity of a native frost to its content window.
    fn opacity(&self, _client: HWND, _alpha: u8) {}
    /// Creates the glass under `client`; `false` when it was refused.
    fn open(&self, client: HWND, shape: Shape, radius: f32, material: &GlassMaterial) -> bool;
    /// Opens with optional optics; old backdrops can continue implementing only `open`.
    fn open_styled(
        &self,
        client: HWND,
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
    fn style(&self, _client: HWND, _style: &GlassStyle) {}
    /// Place.
    fn place(&self, client: HWND, shape: Shape, radius: f32);
    /// Material.
    fn material(&self, client: HWND, material: &GlassMaterial);
    /// Close.
    fn close(&self, client: HWND);
}

thread_local! {
    static BACKDROP: RefCell<Option<Rc<dyn Backdrop>>> = const { RefCell::new(None) };
    static SYSTEM: Rc<super::system_backdrop::SystemBackdrop> = Rc::default();
}

/// Sends this thread's glass requests to `backdrop` from now on. An installed backdrop always
/// wins over the built-in system one, available or not (mypaper's engine).
pub fn install(backdrop: Rc<dyn Backdrop>) {
    BACKDROP.with_borrow_mut(|b| *b = Some(backdrop));
}

/// The installed backdrop, if any.
pub(crate) fn installed() -> Option<Rc<dyn Backdrop>> {
    BACKDROP.with_borrow(Clone::clone)
}

/// Where this thread's glass goes: the installed backdrop, else the system one (which makes
/// nothing until a surface opens glass on it).
pub(crate) fn backdrop() -> Option<Rc<dyn Backdrop>> {
    installed().or_else(|| Some(system() as Rc<dyn Backdrop>))
}

/// This thread's built-in system backdrop, whether or not it is in use.
pub fn system() -> Rc<super::system_backdrop::SystemBackdrop> {
    SYSTEM.with(Rc::clone)
}

/// Whether the system backdrop shows `material` in full: it is available, and the material has
/// no lens (it would get the frost alone).
pub(crate) fn system_serves(material: &GlassMaterial) -> bool {
    !material.lensed() && SYSTEM.with(|s| s.available())
}
