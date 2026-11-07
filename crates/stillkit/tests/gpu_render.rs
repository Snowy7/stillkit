#![cfg(all(windows, feature = "gpu"))]
//! GPU mode draws the same picture as the CPU: one drawing code (`Painter`) on a hardware target.
//! Hardware Direct2D antialiases edges and blends text a little differently, so pixels may
//! differ by a few levels on edges; flat areas match exactly (docs/ui.md, "Render modes").

use std::sync::Arc;

use stillkit::icons::Icon;
use stillkit::platform::windows::gpu;
use stillkit::platform::{Render, set_render};
use stillkit::{DisplayList, Image, Op, Point, Rect, Rgba, TextStyle, Ui, render};

/// The tolerance: at most `MOST` levels per channel on all but `SHARE` of the channels; the rest are
/// isolated antialiased edge pixels (an arc's square end, an icon's round caps), within `EDGE`.
const EDGE: u8 = 128;
const MOST: u8 = 4;
const SHARE: f64 = 0.005;

fn page(ui: &mut Ui) -> DisplayList {
    let text = TextStyle::new("Segoe UI Variable Text", 14.0);
    let (w, h) = ui.text.measure("Liquid glass, partial redraw", &text);
    let ink = Rgba::new(0.95, 0.95, 0.97, 1.0);
    let image = Arc::new(Image {
        width: 16,
        height: 16,
        pixels: (0..256).flat_map(|i| [(i * 7) as u8, (i * 3) as u8, 200, 255]).collect(),
    });
    let fill = |r, radius, color| Op::Fill { rect: r, radius, color };
    DisplayList {
        ops: vec![
            Op::Shadow { rect: Rect::new(20.0, 20.0, 280.0, 180.0), radius: 18.0, spread: 12.0, drop: 4.0, color: Rgba::new(0.0, 0.0, 0.0, 0.3) },
            fill(Rect::new(20.0, 20.0, 280.0, 180.0), 18.0, Rgba::new(0.12, 0.12, 0.14, 0.92)),
            Op::Stroke { rect: Rect::new(20.5, 20.5, 279.0, 179.0), radius: 18.0, width: 1.0, color: Rgba::new(1.0, 1.0, 1.0, 0.12) },
            Op::Text { rect: Rect::new(36.0, 34.0, w, h), text: "Liquid glass, partial redraw".into(), style: text, color: ink },
            Op::Icon { rect: Rect::new(36.0, 70.0, 24.0, 24.0), icon: Icon::Play, weight: 1.6, color: ink },
            Op::Icon { rect: Rect::new(70.0, 70.0, 24.0, 24.0), icon: Icon::Shuffle, weight: 1.6, color: ink },
            Op::Arc { center: Point::new(250.0, 90.0), radius: 22.0, sweep: 4.0, width: 4.0, color: Rgba::new(0.2, 0.55, 1.0, 1.0) },
            Op::Line { from: Point::new(36.0, 110.0), to: Point::new(284.0, 110.0), width: 1.0, color: Rgba::new(1.0, 1.0, 1.0, 0.2) },
            Op::RoundedImage { rect: Rect::new(36.0, 124.0, 56.0, 56.0), image, opacity: 1.0, source: None, radius: 12.0 },
            Op::RoundedClipped {
                rect: Rect::new(110.0, 124.0, 170.0, 56.0),
                radius: 14.0,
                ops: vec![
                    fill(Rect::new(100.0, 110.0, 200.0, 80.0), 0.0, Rgba::new(0.2, 0.55, 1.0, 1.0)),
                    Op::Clipped { rect: Rect::new(110.0, 150.0, 170.0, 30.0), ops: vec![fill(Rect::new(100.0, 110.0, 200.0, 80.0), 0.0, Rgba::new(1.0, 0.3, 0.3, 0.8))].into() },
                ]
                .into(),
            },
        ],
        ..Default::default()
    }
}

/// (largest difference, share of channels differing by more than `MOST`).
fn compare(a: &Image, b: &Image) -> (u8, f64) {
    let max = a.pixels.iter().zip(&b.pixels).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
    let over = a.pixels.iter().zip(&b.pixels).filter(|(x, y)| x.abs_diff(**y) > MOST).count();
    (max, over as f64 / a.pixels.len() as f64)
}

#[test]
fn the_gpu_draws_the_cpu_picture_within_tolerance() {
    let mut ui = Ui::new().unwrap();
    let list = page(&mut ui);
    let cpu = render(&mut ui, &list, 320, 220).unwrap();
    let Some(gpu) = gpu::render(&mut ui, &list, 320, 220).unwrap() else {
        eprintln!("no hardware adapter: nothing to compare");
        return;
    };
    let (max, share) = compare(&cpu, &gpu);
    eprintln!("GPU vs CPU: max {max} levels, {:.3}% of channels over {MOST}", share * 100.0);
    assert!(max <= EDGE && share <= SHARE, "max {max}, share {share}");
    // Flat interiors are exact.
    for (x, y) in [(200, 60), (150, 165), (240, 140)] {
        assert_eq!(cpu.pixel(x, y), gpu.pixel(x, y), "({x}, {y})");
    }
}

#[test]
fn headless_renders_follow_the_threads_render_mode() {
    let mut ui = Ui::new().unwrap();
    let list = page(&mut ui);
    let cpu = render(&mut ui, &list, 320, 220).unwrap();
    set_render(Render::Gpu);
    let followed = render(&mut ui, &list, 320, 220).unwrap();
    set_render(Render::Cpu);
    if let Some(gpu) = gpu::render(&mut ui, &list, 320, 220).unwrap() {
        assert_eq!(followed.pixels, gpu.pixels, "render() drew on the GPU");
    } else {
        assert_eq!(followed.pixels, cpu.pixels, "no adapter: render() stayed on the CPU");
    }
    assert_eq!(render(&mut ui, &list, 320, 220).unwrap().pixels, cpu.pixels, "and back on the CPU");
}
