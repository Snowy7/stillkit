#![cfg(all(any(target_os = "macos", target_os = "linux"), feature = "gpu"))]
//! A native layer's hole ([`Op::Hole`]) on the vello GPU renderer: paint drawn before it is
//! clipped out of it (that renderer has no destructive blend to erase with), paint drawn
//! after it shows on top, as on the CPU.

#[cfg(target_os = "linux")]
use stillkit::platform::linux::gpu;
#[cfg(target_os = "macos")]
use stillkit::platform::macos::gpu;
use stillkit::{DisplayList, Image, Op, Rect, Rgba, Ui, render};

fn page() -> DisplayList {
    let fill = |rect, color| Op::Fill { rect, radius: 0.0, color };
    DisplayList {
        ops: vec![
            // The window's background, then a panel in a scroll view's clip; the hole cuts
            // both, and an overlay after it draws over the native layer.
            fill(Rect::new(0.0, 0.0, 100.0, 100.0), Rgba::new(1.0, 0.0, 0.0, 1.0)),
            Op::Clipped {
                rect: Rect::new(0.0, 0.0, 100.0, 100.0),
                ops: vec![
                    fill(Rect::new(10.0, 10.0, 80.0, 80.0), Rgba::new(0.0, 1.0, 0.0, 1.0)),
                    Op::Hole { rect: Rect::new(20.0, 20.0, 60.0, 60.0) },
                    fill(Rect::new(40.0, 40.0, 10.0, 10.0), Rgba::new(0.0, 0.0, 1.0, 1.0)),
                ]
                .into(),
            },
        ],
        ..Default::default()
    }
}

/// The pixel at (x, y) as (r, g, b, a), from premultiplied BGRA.
fn at(image: &Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.width + x) * 4) as usize;
    let [b, g, r, a] = [image.pixels[i], image.pixels[i + 1], image.pixels[i + 2], image.pixels[i + 3]];
    [r, g, b, a]
}

fn check(image: &Image, renderer: &str) {
    assert_eq!(at(image, 5, 5), [255, 0, 0, 255], "{renderer}: background outside the panel");
    assert_eq!(at(image, 15, 15), [0, 255, 0, 255], "{renderer}: panel outside the hole");
    assert_eq!(at(image, 30, 30)[3], 0, "{renderer}: the hole is transparent");
    assert_eq!(at(image, 70, 70)[3], 0, "{renderer}: all of it");
    assert_eq!(at(image, 45, 45), [0, 0, 255, 255], "{renderer}: an overlay drawn after it shows");
}

#[test]
fn a_hole_cuts_paint_drawn_before_it_and_shows_paint_drawn_after() {
    let mut ui = Ui::new().unwrap();
    let list = page();
    check(&gpu::render(&mut ui, &list, 100, 100).unwrap(), "GPU");
    check(&render(&mut ui, &list, 100, 100).unwrap(), "CPU");
}
