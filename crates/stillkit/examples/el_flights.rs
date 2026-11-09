//! Click the card to move it between parents, interrupting its flight on another click.
use stillkit::el::{self, App, El, col, el, row, text, token};
use stillkit::{Kind, Scheme, Theme};
fn view(right: &bool) -> El<bool> {
    let card = el()
        .key("card")
        .flight("hero")
        .w(if *right { 180.0 } else { 100.0 })
        .h(100.0)
        .radius(if *right { 32.0 } else { 8.0 })
        .fill(if *right { token::ACCENT } else { token::CARD })
        .opacity(if *right { 0.8 } else { 1.0 })
        .on_press(|right: &mut bool| *right = !*right)
        .child(text("Click to fly"));
    let mut left = col().key("left").w(160.0).h(220.0);
    let mut destination = col().key("right").w(200.0).h(220.0).pad_y(80.0);
    if *right {
        destination = destination.child(card);
    } else {
        left = left.child(card);
    }
    row().pad(24.0).gap(24.0).child(left).child(destination)
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Matched flights",
        size: (440.0, 300.0),
        kind: Kind::App { min: (440, 300) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: false,
        view,
    })
}
