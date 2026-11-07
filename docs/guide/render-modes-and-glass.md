# Render modes and glass

Cpu is the default: software Direct2D/WIC on Windows, vello_cpu on Linux, no hardware GPU driver. Gpu requires the gpu feature: hardware Direct2D/DirectComposition on Windows and Vello Hybrid (vello_gpu) on Vulkan on Linux. Without the feature all modes resolve to CPU.

Auto resolves at surface creation: CPU without glass; CPU with an installed external backdrop; GPU when glass is requested and no provider exists. Set the thread default before el::run or use STILLKIT_RENDER=cpu|gpu|auto. Surface::with_render overrides it for one surface. Keep small CPU widgets beside GPU panels.

A Liquid theme changes tokens, but el::run currently does not attach glass bodies automatically. Actual lenses use Surface::set_glass or set_glass_group. glass::install registers an external Backdrop to reuse an existing renderer process. With no provider, use the theme's tinted fill. Windows currently refracts wallpaper even under app windows, rather than capturing arbitrary underlying app content. See the Linux backend report for its app-backdrop path.

Liquid merging is a smooth signed-distance union: bodies in a group fuse within its spacing threshold and spring to updated frames. The complete [glass_merge.rs](../../crates/stillkit/examples/glass_merge.rs) demonstrates Surface hooks. A static kit card is not a lens.

Measured Windows 360 x 220 panel, Ryzen 7800X3D / RTX 4070 SUPER, 2026-11-07 (private MB):

| Configuration | Radeon iGPU | RTX 4070 SUPER |
| --- | ---: | ---: |
| CPU fill | 6.4 | 6.4 |
| GPU without glass | 29.8 | 58.1 |
| CPU + glass process (total) | 228.4 | 168.5 |
| GPU + in-process glass | 251.8 | 178.3 |

CPU drag frame median/p95: 0.282/0.662 ms. GPU + glass: 0.237/0.318 ms iGPU, 0.162/0.214 ms RTX. Driver threads can wake while UI is idle: RTX + glass recorded 5,621 wakes/30 s and 0.312% CPU. A first CPU surface beside GPU adds 1.8-2.5 MB; later ones about 0.1 MB. These are measurements, not universal budgets. [Conditions and Linux results](../ui.md#render-modes).

The executable below deliberately chooses CPU:

```rust
use stillkit::el::{self, App, El, col, text};
use stillkit::{Kind, Scheme, Theme};
type State = ();
fn view(s: &State) -> El<State> {
    let _ = s;
    col()
        .pad(24.)
        .child(text("CPU by default; opt into GPU per surface"))
}
fn main() -> anyhow::Result<()> {
    stillkit::platform::set_render(stillkit::platform::Render::Cpu);
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: (),
        view,
    })
}

/// A custom host can attach merging glass below its own display list.
/// In a CPU-only build this surface remains CPU and needs an external backdrop.
pub fn glass_panel() -> anyhow::Result<stillkit::Surface> {
    use stillkit::glass::{Glass, GlassMaterial};
    use stillkit::platform::Render;
    use stillkit::{Rect, Rgba, Role, Surface};
    let panel = Surface::with_render(
        Kind::Popup { activate: false },
        "Glass panel",
        Role::Dialog,
        Render::Gpu,
    )?;
    let material = GlassMaterial {
        clarity: 0.85,
        blur: 6.,
        refraction: 0.7,
        highlight: 0.5,
        tint: Rgba::TRANSPARENT,
    };
    let body = |x| Glass {
        body: Rect::new(x, 20., 120., 120.),
        radius: 60.,
        material,
    };
    panel.set_glass_group(&[body(20.), body(144.)], 48.);
    Ok(panel)
}
```

[Executable source](../../crates/stillkit/examples/guide_render_modes_and_glass.rs).


[All guides](../../README.md#guides) | [Element reference](../el.md)
