use std::time::Duration;
use stillkit::el::{self, App, El, col};
use stillkit::{Kind, Scheme, Theme};
use stillkit_kit::{Button, Dialog, Toast, Toaster};
#[derive(Default)]
struct State {
    dialog: bool,
    saved: bool,
}
fn view(s: &State) -> El<State> {
    let mut toasts = Toaster::new();
    if s.saved {
        toasts = toasts.child(
            Toast::new("saved", "Settings saved")
                .timeout(Duration::from_secs(3), |s: &mut State| s.saved = false),
        );
    }
    col()
        .pad(24.)
        .child(
            Button::new("Save")
                .key("save")
                .on_press(|s: &mut State| s.dialog = true)
                .build(),
        )
        .overlay(
            Dialog::new(
                "Save settings?",
                "Apply your preferences.",
                |s: &mut State| s.dialog = false,
            )
            .confirm("Save", |s| s.saved = true)
            .build()
            .open(s.dialog),
        )
        .overlay(toasts.build())
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Overlays",
        size: (500., 360.),
        kind: Kind::App { min: (400, 300) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State::default(),
        view,
    })
}
