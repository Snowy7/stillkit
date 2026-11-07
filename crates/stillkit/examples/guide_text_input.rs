use stillkit::el::{self, App, El, col, text, text_area, text_input, token};
use stillkit::{Kind, Scheme, Theme};
#[derive(Default)]
struct State {
    name: String,
    notes: String,
    submitted: bool,
}
fn view(s: &State) -> El<State> {
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(text("Display name"))
        .child(
            text_input("name")
                .label("Display name")
                .placeholder("Your name")
                .max_len(32)
                .value(s.name.as_str())
                .pad(10.)
                .fill(token::WELL)
                .on_input(|s: &mut State, value| s.name = value.into())
                .on_submit(|s: &mut State, _| s.submitted = true),
        )
        .child(
            text_area("notes")
                .label("Notes")
                .rows(4)
                .value(s.notes.as_str())
                .pad(10.)
                .fill(token::WELL)
                .on_input(|s: &mut State, value| s.notes = value.into()),
        )
        .child(text(if s.submitted {
            "Submitted"
        } else {
            "Enter submits the name"
        }))
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State::default(),
        view,
    })
}
