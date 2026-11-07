#![cfg(all(windows, feature = "gpu"))]
//! The opposite of `no_gpu_driver.rs`, for GPU mode: CPU surfaces still load no GPU driver with
//! the `gpu` feature compiled in, and the first GPU surface is what brings the adapter's driver
//! in. Its own test binary, so nothing else is loaded.

use stillkit::platform::windows::gpu;
use stillkit::platform::{Render, set_render};
use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ui, modules};

fn frame(ui: &mut Ui) -> DisplayList {
    let style = TextStyle::new("Segoe UI Variable Text", 14.0);
    let (w, h) = ui.text.measure("Refresh", &style);
    DisplayList {
        ops: vec![
            Op::Fill { rect: Rect::new(0.0, 0.0, 200.0, 40.0), radius: 8.0, color: Rgba::new(0.1, 0.1, 0.1, 0.9) },
            Op::Text { rect: Rect::new(10.0, 10.0, w, h), text: "Refresh".into(), style, color: Rgba::new(1.0, 1.0, 1.0, 1.0) },
        ],
        ..Default::default()
    }
}

#[test]
fn cpu_surfaces_load_no_driver_and_the_first_gpu_surface_loads_one() {
    let mut ui = Ui::new().unwrap();
    // `Auto` without glass wanted is CPU.
    set_render(Render::Auto);
    let mut cpu = Surface::new(Kind::Popup { activate: false }, "cpu", Role::Menu).unwrap();
    assert_eq!(cpu.render(), Render::Cpu);
    let list = frame(&mut ui);
    cpu.present(&mut ui, (0, 0), (200, 40), list).unwrap();
    assert_eq!(modules::gpu_drivers(), Vec::<String>::new(), "a CPU surface loaded a driver");
    assert!(!modules::loaded().iter().any(|m| m == "d3d12.dll"), "wgpu stays out until glass is asked for");

    let Some(adapter) = gpu::adapter(gpu::power()) else {
        eprintln!("no hardware adapter: GPU mode falls back to the CPU here");
        return;
    };
    let mut surface = Surface::with_render(Kind::Popup { activate: false }, "gpu", Role::Menu, Render::Gpu).unwrap();
    assert_eq!(surface.render(), Render::Gpu);
    let list = frame(&mut ui);
    assert!(surface.present(&mut ui, (0, 0), (200, 40), list.clone()).unwrap(), "the first frame draws");
    assert!(!surface.present(&mut ui, (0, 0), (200, 40), list).unwrap(), "an identical frame draws nothing");
    let loaded = modules::loaded();
    assert!(
        !modules::gpu_drivers().is_empty(),
        "GPU mode on {} loaded no vendor driver; all modules: {loaded:?}",
        adapter.name
    );
    assert!(loaded.iter().any(|m| m == "dcomp.dll"), "DirectComposition presents it");
    // The CPU surface beside it keeps drawing in software.
    let list = frame(&mut ui);
    cpu.present(&mut ui, (0, 0), (200, 41), list).unwrap();
    assert_eq!(cpu.render(), Render::Cpu);
}
