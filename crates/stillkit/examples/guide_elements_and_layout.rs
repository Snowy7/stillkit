use stillkit::el::{self, App, Axis, El, col, grid, row, text, token};
use stillkit::{Kind, Scheme, Theme};
type State = ();
fn view(s: &State) -> El<State> {
    let _ = s;
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(text("Workspace").font(stillkit::TextRole::Title1))
        .child(
            row()
                .gap(16.)
                .grow(1.)
                .child(col().w(140.).child(text("Navigation")))
                .child(
                    grid(2)
                        .grow(1.)
                        .gap(12.)
                        .scroll(Axis::Vertical)
                        .children((1..=12).map(|i| text(format!("Project {i}")))),
                ),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: (),
        view,
    })
}
