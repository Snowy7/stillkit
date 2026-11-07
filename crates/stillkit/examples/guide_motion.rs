use stillkit::el::{self, App, El, Transition, col, el, text, token};
use stillkit::{Kind, Scheme, Theme};
type State = bool;
fn view(s: &State) -> El<State> {
    col()
        .animate()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(
            text("Resize")
                .key("resize")
                .on_press(|s: &mut State| *s = !*s),
        )
        .child(
            el().key("card")
                .w(if *s { 280. } else { 140. })
                .h(80.)
                .fill(token::CARD)
                .radius(token::RADIUS_CARD)
                .animate(),
        )
        .children(s.then(|| {
            text("Details")
                .key("details")
                .transition(Transition::slide(0., 16.))
        }))
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: false,
        view,
    })
}
