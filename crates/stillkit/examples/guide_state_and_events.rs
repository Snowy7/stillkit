use stillkit::el::{self, App, El, Handled, Tx, col, row, text, token};
use stillkit::{Kind, Scheme, Theme};
#[derive(Default)]
struct State {
    count: u32,
    dark: bool,
}
fn view(s: &State) -> El<State> {
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .on_key(|s: &mut State, key, tx| {
            if key == el::KeyCode(0x1B) {
                s.count = 0;
                tx.instant();
                Handled::Yes
            } else {
                Handled::No
            }
        })
        .child(text(format!("{} clicks (Escape resets)", s.count)))
        .child(
            row()
                .focus_group(el::Orientation::Horizontal)
                .child(
                    text("Add")
                        .key("add")
                        .on_press(|s: &mut State| s.count += 1),
                )
                .child(
                    text("Theme")
                        .key("theme")
                        .on_press_with(|s: &mut State, tx: &mut Tx| {
                            s.dark = !s.dark;
                            tx.set_theme(Theme::solid(
                                if s.dark { Scheme::Dark } else { Scheme::Light },
                                stillkit::theme::SYSTEM_BLUE,
                            ));
                        }),
                ),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State {
            dark: true,
            ..Default::default()
        },
        view,
    })
}
