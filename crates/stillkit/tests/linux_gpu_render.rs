#![cfg(all(target_os = "linux", feature = "gpu"))]
//! GPU mode draws the same picture as the CPU: one drawing code (`Painter`) on a hardware target.
//! Hardware Direct2D antialiases edges and blends text a little differently, so pixels may
//! differ by a few levels on edges; flat areas match exactly (docs/ui.md, "Render modes").

use std::sync::Arc;

use stillkit::icons::Icon;
use stillkit::platform::linux::gpu;

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
        pixels: (0..256)
            .flat_map(|i| [(i * 7) as u8, (i * 3) as u8, 200, 255])
            .collect(),
    });
    let fill = |r, radius, color| Op::Fill {
        rect: r,
        radius,
        color,
    };
    DisplayList {
        ops: vec![
            Op::Shadow {
                rect: Rect::new(20.0, 20.0, 280.0, 180.0),
                radius: 18.0,
                spread: 12.0,
                drop: 4.0,
                color: Rgba::new(0.0, 0.0, 0.0, 0.3),
            },
            fill(
                Rect::new(20.0, 20.0, 280.0, 180.0),
                18.0,
                Rgba::new(0.12, 0.12, 0.14, 0.92),
            ),
            Op::Stroke {
                rect: Rect::new(20.5, 20.5, 279.0, 179.0),
                radius: 18.0,
                width: 1.0,
                color: Rgba::new(1.0, 1.0, 1.0, 0.12),
            },
            Op::Text {
                rect: Rect::new(36.0, 34.0, w, h),
                text: "Liquid glass, partial redraw".into(),
                style: text,
                color: ink,
            },
            Op::Icon {
                rect: Rect::new(36.0, 70.0, 24.0, 24.0),
                icon: Icon::Play,
                weight: 1.6,
                color: ink,
            },
            Op::Icon {
                rect: Rect::new(70.0, 70.0, 24.0, 24.0),
                icon: Icon::Shuffle,
                weight: 1.6,
                color: ink,
            },
            Op::Arc {
                center: Point::new(250.0, 90.0),
                radius: 22.0,
                sweep: 4.0,
                width: 4.0,
                color: Rgba::new(0.2, 0.55, 1.0, 1.0),
            },
            Op::Line {
                from: Point::new(36.0, 110.0),
                to: Point::new(284.0, 110.0),
                width: 1.0,
                color: Rgba::new(1.0, 1.0, 1.0, 0.2),
            },
            Op::RoundedImage {
                rect: Rect::new(36.0, 124.0, 56.0, 56.0),
                image,
                opacity: 1.0,
                source: None,
                radius: 12.0,
            },
            Op::RoundedClipped {
                rect: Rect::new(110.0, 124.0, 170.0, 56.0),
                radius: 14.0,
                ops: vec![
                    fill(
                        Rect::new(100.0, 110.0, 200.0, 80.0),
                        0.0,
                        Rgba::new(0.2, 0.55, 1.0, 1.0),
                    ),
                    Op::Clipped {
                        rect: Rect::new(110.0, 150.0, 170.0, 30.0),
                        ops: vec![fill(
                            Rect::new(100.0, 110.0, 200.0, 80.0),
                            0.0,
                            Rgba::new(1.0, 0.3, 0.3, 0.8),
                        )]
                        .into(),
                    },
                ]
                .into(),
            },
        ],
        ..Default::default()
    }
}

/// (largest difference, share of channels differing by more than `MOST`).
fn compare(a: &Image, b: &Image) -> (u8, f64) {
    let max = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0);
    let over = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .filter(|(x, y)| x.abs_diff(**y) > MOST)
        .count();
    (max, over as f64 / a.pixels.len() as f64)
}

#[test]
fn the_gpu_draws_the_cpu_picture_within_tolerance() {
    let mut ui = Ui::new().unwrap();
    let list = page(&mut ui);
    let cpu = render(&mut ui, &list, 320, 220).unwrap();
    let gpu = gpu::render(&mut ui, &list, 320, 220).unwrap();
    let (max, share) = compare(&cpu, &gpu);
    eprintln!(
        "GPU vs CPU: max {max} levels, {:.3}% of channels over {MOST}",
        share * 100.0
    );
    assert!(max <= EDGE && share <= SHARE, "max {max}, share {share}");
    // Flat interiors are exact.
    for (x, y) in [(200, 60), (150, 165), (240, 140)] {
        assert!(
            cpu.pixel(x, y)
                .iter()
                .zip(gpu.pixel(x, y))
                .all(|(a, b)| a.abs_diff(b) <= 1),
            "flat blend rounding at ({x}, {y})"
        );
    }
}

#[test]
fn partial_gpu_updates_equal_full_gpu_render_and_headless_follows_mode() {
    use stillkit::platform::{Render, set_render};
    let mut ui = Ui::new().unwrap();
    let before = page(&mut ui);
    let mut after = before.clone();
    after.ops.push(Op::Fill {
        rect: Rect::new(120., 80., 40., 30.),
        radius: 6.,
        color: Rgba::new(1., 0., 0., 0.7),
    });
    let mut canvas = gpu::Canvas::new(320, 220).unwrap();
    canvas
        .draw(&mut ui, &before, stillkit::PixelRect::full(320, 220))
        .unwrap();
    canvas
        .draw(
            &mut ui,
            &after,
            after.damage(&before).unwrap().pixels(320, 220).unwrap(),
        )
        .unwrap();
    let full = gpu::render(&mut ui, &after, 320, 220).unwrap();
    assert_eq!(canvas.image().unwrap().pixels, full.pixels);
    set_render(Render::Gpu);
    assert_eq!(
        render(&mut ui, &after, 320, 220).unwrap().pixels,
        full.pixels
    );
    set_render(Render::Cpu);
}
