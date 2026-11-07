#![cfg(all(target_os = "linux", feature = "gpu"))]
//! Runs in its own process: compiling GPU support must not initialize it.
use stillkit::platform::{Render, set_render};
use stillkit::{DisplayList, Op, Rect, Rgba, Ui, modules};
#[test]
fn only_a_gpu_request_loads_vulkan_and_cpu_rendering_stays_cpu() {
    set_render(Render::Cpu);
    let mut ui = Ui::new().unwrap();
    let list = DisplayList {
        ops: vec![Op::Fill {
            rect: Rect::new(0., 0., 64., 64.),
            radius: 8.,
            color: Rgba::new(0.2, 0.4, 0.8, 1.),
        }],
        ..Default::default()
    };
    let cpu = stillkit::render(&mut ui, &list, 64, 64).unwrap();
    assert!(!modules::loaded().iter().any(|m| modules::is_gpu_driver(m)));
    set_render(Render::Gpu);
    let gpu = stillkit::render(&mut ui, &list, 64, 64).unwrap();
    assert!(gpu.pixels.iter().any(|p| *p != 0));
    assert!(modules::loaded().iter().any(|m| m.contains("libvulkan")));
    assert!(
        !modules::loaded()
            .iter()
            .any(|m| m.contains("libEGL") || m.contains("libGL.so") || m.contains("radeonsi")),
        "only the Vulkan backend is enabled"
    );
    set_render(Render::Cpu);
    assert_eq!(
        stillkit::render(&mut ui, &list, 64, 64).unwrap().pixels,
        cpu.pixels
    );
    assert_eq!(Render::Auto.resolve(false, false), Render::Cpu);
    assert_eq!(Render::Auto.resolve(true, false), Render::Gpu);
    assert_eq!(Render::Auto.resolve(true, true), Render::Cpu);
}
