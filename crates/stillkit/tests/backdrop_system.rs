#![cfg(windows)]
//! The built-in system backdrop (docs/ui.md, "System backdrop"): DWM's frost in a window of its
//! own directly under a CPU surface, made on the first open and gone with the last close; and an
//! installed backdrop always winning over it. Live windows show (never activated, no input).
//! The frost checks skip where the system backdrop is unavailable (Windows 10, transparency off).

use std::cell::Cell;
use std::rc::Rc;

use stillkit::glass::{self, Backdrop, Glass, GlassMaterial, Shape};
use stillkit::platform::Render;
use stillkit::platform::windows::glass::system;
use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, Ui};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{GW_HWNDNEXT, GetWindow, GetWindowRect, IsWindow, IsWindowVisible};

const SIZE: (u32, u32) = (200, 120);
const FROST: GlassMaterial = GlassMaterial {
    clarity: 0.35,
    blur: 24.0,
    refraction: 0.0,
    highlight: 0.0,
    tint: Rgba::new(0.1, 0.1, 0.12, 0.4),
};

fn body() -> DisplayList {
    DisplayList {
        ops: vec![Op::Fill { rect: Rect::new(0.0, 0.0, 200.0, 120.0), radius: 16.0, color: Rgba::HIT }],
        ..Default::default()
    }
}

fn glass(body: Rect) -> Option<Glass> {
    Some(Glass { body, radius: 16.0, material: FROST })
}

fn shown(ui: &mut Ui, at: (i32, i32)) -> Surface {
    let mut s = Surface::new(Kind::Popup { activate: false }, "frost test", Role::Dialog).unwrap();
    s.present(ui, at, SIZE, body()).unwrap();
    s.show();
    s.set_glass(glass(Rect::new(0.0, 0.0, 200.0, 120.0)));
    s
}

fn rect(window: HWND) -> (i32, i32, i32, i32) {
    let mut r = RECT::default();
    // SAFETY: a query into a local.
    unsafe { GetWindowRect(window, &mut r) }.unwrap();
    (r.left, r.top, r.right, r.bottom)
}

fn under(window: HWND) -> Option<HWND> {
    // SAFETY: a z-order query.
    unsafe { GetWindow(window, GW_HWNDNEXT) }.ok()
}

#[test]
fn frost_sits_directly_under_its_surface_and_follows_it() {
    let mut ui = Ui::new().unwrap();
    let system = system();
    if !system.available() {
        eprintln!("the system backdrop is unavailable here");
        return;
    }
    let mut s = shown(&mut ui, (120, 140));
    assert!(s.has_glass() && s.glass_available() && system.active());
    let frost = system.window(s.hwnd()).expect("a frost window");
    assert_eq!(under(s.hwnd()), Some(frost), "directly under its surface");
    assert_eq!(rect(frost), (120, 140, 320, 260), "the body on screen");

    s.move_to((400, 300)).unwrap();
    assert_eq!(rect(frost), (400, 300, 600, 420), "placed with the move");
    s.set_glass(glass(Rect::new(10.0, 10.0, 100.0, 50.0)));
    assert_eq!(rect(frost), (410, 310, 510, 360), "reshaped");
    assert_eq!(under(s.hwnd()), Some(frost));

    // The last frost closed: everything goes, and comes back on the next show.
    s.hide();
    assert!(!system.active() && !s.has_glass());
    // SAFETY: a liveness query of a handle we no longer own.
    assert!(!unsafe { IsWindow(Some(frost)) }.as_bool(), "the frost window was destroyed");
    s.show();
    let again = system.window(s.hwnd()).expect("reopened on show");
    assert_eq!(under(s.hwnd()), Some(again));
    drop(s);
    assert!(!system.active());
}

#[test]
fn the_compositor_stays_while_any_frost_is_open() {
    let mut ui = Ui::new().unwrap();
    let system = system();
    if !system.available() {
        eprintln!("the system backdrop is unavailable here");
        return;
    }
    let a = shown(&mut ui, (100, 100));
    let b = shown(&mut ui, (360, 100));
    let frost_a = system.window(a.hwnd()).unwrap();
    a.hide();
    assert!(system.active(), "b's frost is still open");
    assert!(system.window(a.hwnd()).is_none());
    // SAFETY: a visibility query of our own window.
    assert!(!unsafe { IsWindowVisible(frost_a) }.as_bool(), "a's frost hides with a");
    a.show();
    assert_eq!(system.window(a.hwnd()), Some(frost_a), "kept while closed, shown again");
    a.hide();
    b.hide();
    assert!(!system.active());
}

/// An installed backdrop that records what it is asked.
#[derive(Default)]
struct Recorder {
    opened: Cell<u32>,
}

impl Backdrop for Recorder {
    fn available(&self) -> bool {
        false
    }
    fn open(&self, _: HWND, _: Shape, _: f32, _: &GlassMaterial) -> bool {
        self.opened.set(self.opened.get() + 1);
        true
    }
    fn place(&self, _: HWND, _: Shape, _: f32) {}
    fn material(&self, _: HWND, _: &GlassMaterial) {}
    fn close(&self, _: HWND) {}
}

#[test]
fn an_installed_backdrop_always_wins_even_when_unavailable() {
    let mut ui = Ui::new().unwrap();
    let recorder = Rc::new(Recorder::default());
    glass::install(recorder.clone());
    let s = shown(&mut ui, (100, 100));
    assert_eq!(recorder.opened.get(), 1, "the installed backdrop got the glass");
    assert!(!system().active(), "the system backdrop made nothing");
    assert!(!glass::available() && !s.glass_available(), "views draw their fill: the engine is gone");
    glass::set_style(Some(FROST));
    assert!(glass::cpu_backdrop());
    assert_eq!(Render::Auto.resolve(glass::wanted(), glass::cpu_backdrop()), Render::Cpu);
}

#[test]
fn auto_goes_gpu_only_for_glass_no_cpu_backdrop_draws_in_full() {
    let gpu = if cfg!(feature = "gpu") { Render::Gpu } else { Render::Cpu };
    assert_eq!(Render::Auto.resolve(false, false), Render::Cpu);
    assert_eq!(Render::Auto.resolve(true, true), Render::Cpu);
    assert_eq!(Render::Auto.resolve(true, false), gpu);
    // No backdrop installed: frost is the system's when it is available; a lens never is.
    glass::set_style(Some(FROST));
    assert_eq!(glass::cpu_backdrop(), system().available());
    glass::set_style(Some(GlassMaterial { refraction: 0.6, highlight: 0.75, ..FROST }));
    assert!(!glass::cpu_backdrop(), "a lensed material needs the GPU or an engine");
    glass::set_style(None);
    assert!(!glass::cpu_backdrop());
}

#[test]
fn app_windows_never_get_the_system_frost() {
    let app = Surface::new(Kind::App { min: (200, 120) }, "app", Role::Window).unwrap();
    assert!(!app.glass_available(), "a CPU app window draws opaque");
    app.set_glass(glass(Rect::new(0.0, 0.0, 200.0, 120.0)));
    assert!(!app.has_glass() && !system().active());
}
