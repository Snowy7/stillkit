//! Where a CPU surface's glass requests go ([`Backdrop`]): an app's own provider
//! ([`install`]), or the system's frost ([`install_system`]): an `NSGlassEffectView` (macOS 26)
//! or `NSVisualEffectView` under the surface's content, shaped and rounded to its glass body
//! and tinted with the material. An installed provider always wins over the system one.
//! Neither draws a lens (refraction, rim): that is the GPU mode's in-process glass.
use std::{cell::RefCell, collections::HashMap, rc::Rc};
type WindowId = usize;
use crate::glass::{GlassMaterial, GlassStyle, Shape};
use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSGlassEffectView, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::NSRect;

/// Where glass requests go. Every call is for `client`, the surface's id
/// ([`Surface::id`](crate::Surface::id) as a number).
pub trait Backdrop {
    /// Whether glass can show now. Views draw a fill of their own when not.
    fn available(&self) -> bool;
    /// Creates the glass under `client`; `false` when it was refused.
    fn open(&self, client: WindowId, shape: Shape, radius: f32, material: &GlassMaterial) -> bool;
    /// Opens with optional optics; old backdrops can continue implementing only `open`.
    fn open_styled(&self, client: WindowId, shape: Shape, radius: f32, material: &GlassMaterial, style: &GlassStyle) -> bool {
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
    static SYSTEM: RefCell<Option<Rc<dyn Backdrop>>> = const { RefCell::new(None) };
}

/// Sends this thread's glass requests to `backdrop` from now on (over the system's).
pub fn install(backdrop: Rc<dyn Backdrop>) {
    BACKDROP.with_borrow_mut(|b| *b = Some(backdrop));
}

/// Lets this thread's CPU surfaces show the system's frost when no provider is installed.
pub fn install_system() {
    SYSTEM.with_borrow_mut(|b| *b = Some(Rc::new(SystemBackdrop::default())));
}

pub(crate) fn backdrop() -> Option<Rc<dyn Backdrop>> {
    BACKDROP.with_borrow(Clone::clone).or_else(|| SYSTEM.with_borrow(Clone::clone))
}

/// AppKit's own material views under a surface's content view, one per surface.
#[derive(Default)]
pub struct SystemBackdrop {
    views: RefCell<HashMap<WindowId, Plate>>,
}

/// One surface's frost: the effect view, and the tint over it (the visual effect view has none).
struct Plate {
    effect: Retained<NSView>,
    tint: Option<Retained<NSView>>,
    glass: bool,
}

impl SystemBackdrop {
    /// `shape` (screen px) in the surface's container (window points).
    fn frame(client: WindowId, shape: Shape) -> Option<(Retained<super::view::View>, NSRect, f64)> {
        let view = super::surface::view(super::surface::SurfaceId::from_raw(client))?;
        let window = view.window()?;
        let s = super::screen::primary().scale;
        let global = super::screen::frame_at((shape.x, shape.y), (shape.width as f64 / s, shape.height as f64 / s));
        let in_window = window.convertRectFromScreen(global);
        // SAFETY: a live view's superview, the surface's container.
        let container = unsafe { view.superview() }?;
        let local = container.convertRect_fromView(in_window, None);
        Some((view, local, window.backingScaleFactor()))
    }

    fn tint(plate: &Plate, material: &GlassMaterial) {
        let t = material.tint;
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(t.r as f64, t.g as f64, t.b as f64, t.a as f64);
        if plate.glass {
            if let Some(glass) = plate.effect.downcast_ref::<NSGlassEffectView>() {
                glass.setTintColor((t.a > 0.0).then_some(&*color));
            }
        } else if let Some(tint) = &plate.tint
            && let Some(layer) = tint.layer()
        {
            layer.setBackgroundColor(Some(&color.CGColor()));
        }
        // Clear glass shows more of what is behind it: the frost fades with clarity.
        plate.effect.setAlphaValue(1.0 - 0.6 * material.clarity.clamp(0.0, 1.0) as f64);
    }

    fn round(plate: &Plate, radius: f64) {
        if plate.glass {
            if let Some(glass) = plate.effect.downcast_ref::<NSGlassEffectView>() {
                glass.setCornerRadius(radius);
            }
        } else if let Some(layer) = plate.effect.layer() {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(true);
        }
    }
}

impl Backdrop for SystemBackdrop {
    fn available(&self) -> bool {
        MainThreadMarker::new().is_some()
    }

    fn open(&self, client: WindowId, shape: Shape, radius: f32, material: &GlassMaterial) -> bool {
        let Some(mtm) = MainThreadMarker::new() else { return false };
        self.close(client);
        let Some((view, frame, scale)) = Self::frame(client, shape) else { return false };
        // SAFETY: the surface's container.
        let Some(container) = (unsafe { view.superview() }) else { return false };
        // NSGlassEffectView exists from macOS 26 only; objc2 looks classes up by name.
        let glass = AnyClass::get(c"NSGlassEffectView").is_some();
        let effect: Retained<NSView> = if glass {
            Retained::into_super(NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(mtm), frame))
        } else {
            let v = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), frame);
            v.setMaterial(NSVisualEffectMaterial::HUDWindow);
            v.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            v.setState(NSVisualEffectState::Active);
            v.setWantsLayer(true);
            Retained::into_super(v)
        };
        let tint = (!glass).then(|| {
            let t = NSView::initWithFrame(NSView::alloc(mtm), effect.bounds());
            t.setWantsLayer(true);
            t.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
            effect.addSubview(&t);
            t
        });
        container.addSubview_positioned_relativeTo(&effect, NSWindowOrderingMode::Below, Some(&view));
        let plate = Plate { effect, tint, glass };
        Self::round(&plate, radius as f64 / scale);
        Self::tint(&plate, material);
        self.views.borrow_mut().insert(client, plate);
        true
    }

    fn place(&self, client: WindowId, shape: Shape, radius: f32) {
        let Some((_, frame, scale)) = Self::frame(client, shape) else { return };
        if let Some(plate) = self.views.borrow().get(&client) {
            plate.effect.setFrame(frame);
            Self::round(plate, radius as f64 / scale);
        }
    }

    fn material(&self, client: WindowId, material: &GlassMaterial) {
        if let Some(plate) = self.views.borrow().get(&client) {
            Self::tint(plate, material);
        }
    }

    fn close(&self, client: WindowId) {
        let plate = self.views.borrow_mut().remove(&client);
        if let Some(plate) = plate {
            plate.effect.removeFromSuperview();
        }
    }
}
