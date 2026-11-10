//! Native gradients and draw-time transforms in backgrounds and borders; click to retarget their springs.
use stillkit::el::{self, App, El, Paint, col, el, text};
use stillkit::paint::GradientStop;
use stillkit::{Kind, Rgba, Scheme, Theme};

/// The warm gradient shown before the first click.
pub const WARM: &[GradientStop] = &[
    GradientStop {
        offset: 0.0,
        color: Rgba::new(1.0, 0.25, 0.1, 1.0),
    },
    GradientStop {
        offset: 1.0,
        color: Rgba::new(0.3, 0.1, 0.8, 1.0),
    },
];
/// The cool gradient with inset stops, reached after a click.
pub const COOL: &[GradientStop] = &[
    GradientStop {
        offset: 0.2,
        color: Rgba::new(0.1, 0.8, 0.9, 1.0),
    },
    GradientStop {
        offset: 0.8,
        color: Rgba::new(0.1, 0.2, 0.5, 1.0),
    },
];
/// Three native gradient surfaces sharing the same animated state.
pub fn view(on: &bool) -> El<bool> {
    let stops = if *on { COOL } else { WARM };
    col()
        .pad(20.0)
        .gap(16.0)
        .fill(Rgba::new(0.07, 0.08, 0.1, 1.0))
        .child(
            el().key("linear")
                .w(280.0)
                .h(90.0)
                .radius(18.0)
                .pad(14.0)
                .fill(Paint::Linear {
                    stops,
                    angle: if *on { 1.4 } else { 0.0 },
                })
                .animate()
                .on_press(|on: &mut bool| *on = !*on)
                .child(text("Click to change the gradient")),
        )
        .child(
            el().key("radial")
                .rotate(if *on { 0.22 } else { 0.0 })
                .scale(if *on { 0.85 } else { 1.0 })
                .translate(if *on { 16.0 } else { 0.0 },0.0)
                .origin(0.0,0.5)
                .w(280.0)
                .h(90.0)
                .radius(18.0)
                .pad(14.0)
                .fill(Paint::Radial { stops })
                .animate(),
        )
        .child(
            el().w(280.0)
                .h(70.0)
                .radius(18.0)
                .pad(14.0)
                .border(6.0, Paint::Linear { stops, angle: 0.5 })
                .animate()
                .child(text("Native gradient border")),
        )
        .child(
            el().w(280.0)
                .h(75.0)
                .fill(Paint::Linear { stops, angle: 0.0 })
                .child(
                    el().absolute(20.0, 10.0)
                        .w(240.0)
                        .h(55.0)
                        .radius(12.0)
                        .backdrop_blur(if *on { 12.0 } else { 3.0 })
                        .fill(Rgba::new(1.0, 1.0, 1.0, 0.2))
                        .animate()
                        .child(
                            text("Frosted backdrop")
                                .blur(if *on { 2.0 } else { 0.0 })
                                .animate(),
                        ),
                ),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Paint and motion",
        size: (320.0, 440.0),
        kind: Kind::App { min: (320, 440) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: false,
        view,
    })
}
