#![cfg(all(target_os = "linux", feature = "gpu"))]
//! Explicit live check; run under GNOME's strict new-window focus policy. No activation or
//! input injection. Normal workspace tests remain usable without a Wayland session.
use stillkit::platform::{Render, set_render};
use stillkit::{DisplayList, Glass, GlassMaterial, Image, Kind, Op, Rect, Rgba, Role, Surface, Ui};

#[test]
#[ignore = "requires a live Wayland session with strict new-window focus policy"]
fn mixed_surfaces_resize_remap_and_refract_client_pixels() -> anyhow::Result<()> {
    set_render(Render::Cpu);
    let mut ui = Ui::new()?;
    let mut cpu = Surface::new(
        Kind::Popup { activate: false },
        "stillkit CPU retention check",
        Role::Window,
    )?;
    let mut gpu = Surface::with_render(
        Kind::Popup { activate: false },
        "stillkit Vulkan retention check",
        Role::Window,
        Render::Gpu,
    )?;
    let list = DisplayList {
        ops: vec![Op::Fill {
            rect: Rect::new(12., 12., 80., 40.),
            radius: 8.,
            color: Rgba::new(0.2, 0.6, 1., 0.8),
        }],
        ..Default::default()
    };
    cpu.present(&mut ui, (0, 0), (160, 100), list.clone())?;
    assert!(
        !stillkit::modules::loaded()
            .iter()
            .any(|m| stillkit::modules::is_gpu_driver(m))
    );
    assert_eq!(cpu.render(), Render::Cpu);
    assert_eq!(gpu.render(), Render::Gpu);
    for size in [(160, 100), (240, 180)] {
        assert!(gpu.present(&mut ui, (0, 0), size, list.clone())?);
        assert!(!gpu.present(&mut ui, (0, 0), size, list.clone())?);
        assert_eq!(
            gpu.image()?.pixels,
            stillkit::platform::linux::gpu::render(&mut ui, &list, size.0, size.1)?.pixels
        );
    }
    gpu.hide();
    gpu.show();
    assert!(!gpu.present(&mut ui, (0, 0), (240, 180), list.clone())?);
    gpu.set_alpha(128)?;
    gpu.set_alpha(255)?;
    gpu.set_glass_backdrop(Image {
        width: 1,
        height: 1,
        pixels: vec![70, 110, 150, 255],
    })?;
    gpu.set_glass(Some(Glass {
        body: Rect::new(8., 8., 220., 160.),
        radius: 18.,
        material: GlassMaterial {
            clarity: 0.8,
            blur: 8.,
            refraction: 0.6,
            highlight: 0.5,
            tint: Rgba::TRANSPARENT,
        },
    }));
    assert!(gpu.glass_available());
    assert!(gpu.present(&mut ui, (0, 0), (240, 180), list.clone())?);
    let settle = std::time::Instant::now();
    while settle.elapsed() < std::time::Duration::from_secs(2) {
        while stillkit::next_event().is_some() {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let glass = gpu.image()?;
    assert_ne!(glass.pixel(120, 80), [0, 0, 0, 0]);
    assert!(!gpu.present(&mut ui, (0, 0), (240, 180), list.clone())?);
    cpu.present(&mut ui, (0, 0), (160, 100), list)?;
    assert_eq!(cpu.render(), Render::Cpu);
    Ok(())
}
