use stillkit::el::{self, App, El, col, text, token};
use stillkit::{Kind, Scheme, Theme};
fn view(count: &u32) -> El<u32> {
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(text(format!("Hello, stillkit! {count}")))
        .child(
            text("Click to count")
                .key("count")
                .on_press(|count: &mut u32| *count += 1),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Hello",
        size: (360.0, 240.0),
        kind: Kind::App { min: (240, 160) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: 0,
        view,
    })
}
