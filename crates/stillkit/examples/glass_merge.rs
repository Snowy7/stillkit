//! Liquid merging through the glass hook: two glass bodies on one GPU surface, in one container
//! (`Surface::set_glass_group`). Every second they spring together, fuse through a liquid neck,
//! and pull apart again. The surface's content (two labels) stays on top of the glass.
//!
//! ```text
//! cargo run --release -p stillkit --features gpu --example glass_merge -- [--seconds 8]
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("GPU mode is Windows-only.");
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use std::time::{Duration, Instant};
    use stillkit::glass::{Glass, GlassMaterial, GlassStyle};
    use stillkit::platform::Render;
    use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ui};

    let seconds: u64 = std::env::args()
        .skip_while(|a| a != "--seconds")
        .nth(1)
        .map_or(Ok(8), |s| s.parse())?;
    let mut ui = Ui::new()?;
    let mut surface = Surface::with_render(Kind::Popup { activate: false }, "glass merge", Role::Dialog, Render::Gpu)?;
    anyhow::ensure!(surface.render() == Render::Gpu, "no hardware adapter for GPU mode");
    let material = GlassMaterial { clarity: 0.85, blur: 6.0, refraction: 0.7, highlight: 0.5, tint: Rgba::TRANSPARENT };
    let size = (560u32, 220u32);
    let ball = |x: f32| Glass { body: Rect::new(x, 50.0, 120.0, 120.0), radius: 60.0, material };
    let label = TextStyle::new("Segoe UI Variable Text", 15.0);
    let ink = Rgba::new(1.0, 1.0, 1.0, 0.95);
    let mut ops = Vec::new();
    for (x, text) in [(70.0, "one"), (430.0, "two")] {
        let (w, h) = ui.text.measure(text, &label);
        ops.push(Op::Text { rect: Rect::new(x - w / 2.0 + 30.0, 110.0 - h / 2.0, w, h), text: text.into(), style: label.clone(), color: ink });
    }
    // The glass bodies take clicks through the HIT fill, like any glass surface.
    ops.insert(0, Op::Fill { rect: Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32), radius: 0.0, color: Rgba::HIT });
    surface.present(&mut ui, (300, 300), size, DisplayList { ops, ..Default::default() })?;
    surface.show();
    surface.set_glass_style(GlassStyle::default());
    let started = Instant::now();
    let mut together = false;
    while started.elapsed() < Duration::from_secs(seconds) {
        // Apart: 260 px between them (clean); together: 4 px (fused).
        let (a, b) = if together { (158.0, 282.0) } else { (40.0, 400.0) };
        surface.set_glass_group(&[ball(a), ball(b)], 48.0);
        together = !together;
        let until = Instant::now() + Duration::from_secs(1);
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            stillkit::platform::windows::host::wait_for(left);
        }
    }
    Ok(())
}
