use stillkit::el::{self, App, El, col, text};
use stillkit::{Kind, Scheme, Theme};
type State = ();
fn view(s: &State) -> El<State> {
    let _ = s;
    col().pad(24.).child(text("One view, native hosts"))
}
fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    stillkit::set_identity(stillkit::Identity {
        prefix: "example.guide",
        desktop_classes: &[],
    })
    .expect("set identity before the first window");
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: (),
        view,
    })
}
