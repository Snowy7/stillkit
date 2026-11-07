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
