#![cfg(windows)]
//! Plan Rule 1: drawing with `stillkit` loads no GPU vendor driver, with the system backdrop's
//! frost under the surface too (DWM draws it). An integration test, so it runs in a process of
//! its own with nothing else loaded.

use stillkit::glass::{Backdrop, Glass, GlassMaterial};
use stillkit::platform::windows::glass::system;
use stillkit::{
    DisplayList, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ui, modules, render,
};

#[test]
fn drawing_text_and_shapes_on_a_surface_loads_no_gpu_driver() {
    let mut ui = Ui::new().unwrap();
    let style = TextStyle::new("Segoe UI Variable Text", 14.0);
    let (w, h) = ui.text.measure("Refresh", &style);
    let list = DisplayList {
        ops: vec![
            Op::Fill {
                rect: Rect::new(0.0, 0.0, 200.0, 40.0),
                radius: 8.0,
                color: Rgba::new(0.1, 0.1, 0.1, 0.9),
            },
            Op::Text {
                rect: Rect::new(10.0, 10.0, w, h),
                text: "Refresh".into(),
                style,
                color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            },
        ],
        ..Default::default()
    };
    render(&mut ui, &list, 200, 40).unwrap();
    let mut surface = Surface::new(Kind::Popup { activate: false }, "test", Role::Menu).unwrap();
    surface.present(&mut ui, (0, 0), (200, 40), list).unwrap();
    // One UI's frost from the system backdrop (none is installed), then a move and a reshape.
    surface.show();
    surface.set_glass(Some(Glass {
        body: Rect::new(0.0, 0.0, 200.0, 40.0),
        radius: 8.0,
        material: GlassMaterial { clarity: 0.35, blur: 24.0, refraction: 0.0, highlight: 0.0, tint: Rgba::new(0.1, 0.1, 0.1, 0.4) },
    }));
    if system().available() {
        assert!(surface.has_glass() && system().active(), "the system frost opened");
    } else {
        eprintln!("the system backdrop is unavailable here: CPU mode alone was checked");
    }
    surface.move_to((40, 40)).unwrap();
    pump();

    let loaded = modules::loaded();
    let drivers: Vec<&String> = loaded
        .iter()
        .filter(|m| modules::is_gpu_driver(m))
        .collect();
    assert!(
        drivers.is_empty(),
        "GPU drivers loaded: {drivers:?}\nall modules: {loaded:?}"
    );
    assert!(
        loaded.iter().any(|m| m == "d2d1.dll"),
        "Direct2D did the drawing"
    );
    assert!(!loaded.iter().any(|m| m == "d3d12.dll"), "no Direct3D 12 either");
}

/// Lets the compositor and DWM take the frame (a few message-loop turns).
fn pump() {
    use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, MSG, PM_REMOVE, PeekMessageW};
    for _ in 0..20 {
        // SAFETY: a standard message pump on this thread.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
