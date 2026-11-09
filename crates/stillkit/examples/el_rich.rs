use stillkit::el::{self, App, El, Run};
use stillkit::{Kind, Scheme, Theme};
fn view(_: &()) -> El<()> {
    el::col().pad(24.0).fill(el::token::SURFACE).child(
        el::rich_text([
            Run::new("Rich text ").bold(),
            Run::new("wraps with emphasis, ").italic(),
            Run::new("code").code(),
            Run::new(" and links").link("app:docs"),
        ])
        .selectable()
        .on_link(|_, url| println!("{url}")),
    )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Rich text",
        size: (420.0, 180.0),
        kind: Kind::App { min: (200, 120) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: (),
        view,
    })
}
