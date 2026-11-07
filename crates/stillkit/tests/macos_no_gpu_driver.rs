//! macOS CPU mode loads no GPU driver: drawing (CoreGraphics, CoreText) and a shown window
//! (layer contents, the window server composites) make no Metal device, and every Metal
//! device loads its driver bundle (AGX on Apple silicon, an `MTLDriver` on Intel and AMD),
//! so no driver image may be in the process. Runs on the main thread (AppKit windows need it),
//! hence `harness = false`. `cargo test -p stillkit --test macos_no_gpu_driver` on a Mac.
fn main() {
    #[cfg(target_os = "macos")]
    mac::run();
}

#[cfg(target_os = "macos")]
mod mac {
    use std::time::{Duration, Instant};
    use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ui, modules, next_event, render};

    pub fn run() {
        stillkit::platform::set_render(stillkit::platform::Render::Cpu);
        let mut ui = Ui::new().unwrap();
        let list = DisplayList {
            ops: vec![
                Op::Fill { rect: Rect::new(0., 0., 200., 50.), radius: 8., color: Rgba::new(0.1, 0.2, 0.3, 0.9) },
                Op::Text {
                    rect: Rect::new(10., 10., 180., 30.),
                    text: "CPU semibold".into(),
                    style: TextStyle { weight: 600, ..TextStyle::new("SF Pro", 14.) },
                    color: Rgba::new(1., 1., 1., 1.),
                },
            ],
            ..Default::default()
        };
        let image = render(&mut ui, &list, 200, 50).unwrap();
        assert!(image.pixels.iter().any(|b| *b != 0), "headless CPU render drew nothing");

        let mut surface = Surface::new(Kind::Popup { activate: false }, "no-gpu", Role::Menu).unwrap();
        assert!(surface.present(&mut ui, (100, 100), (200, 50), list).unwrap());
        surface.show();
        // Let the window server show it and AppKit settle.
        let until = Instant::now() + Duration::from_millis(500);
        while Instant::now() < until {
            while next_event().is_some() {}
            stillkit::platform::current::host::wait_for(Duration::from_millis(20));
        }
        surface.hide();

        let drivers: Vec<_> = modules::loaded().into_iter().filter(|m| modules::is_gpu_driver(m)).collect();
        assert!(drivers.is_empty(), "GPU driver images in CPU mode: {drivers:?}");
        println!("macos_no_gpu_driver: ok ({} images, none a GPU driver)", modules::loaded().len());
    }
}
