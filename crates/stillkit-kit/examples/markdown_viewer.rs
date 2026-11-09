//! Local Markdown viewer. Enable `markdown`; without it, shows the same source as plain text.
use stillkit::el::{self, App, El};
use stillkit::{Kind, Scheme, Theme};
const DOCUMENT: &str = "# A local document\n\nParagraphs with **bold**, *italic*, ~~deleted~~ and `inline code`. Read [the guide](app:guide).\n\n## Lists and quotes\n\n3. First item\n   - A nested item with wrapping text\n4. Second item\n\n> Quotes use the current look.\n\n---\n\n| Feature | Status |\n| :--- | ---: |\n| Selection | Ready |\n| Network | None |\n\n```rust\nlet answer = 42;\nprintln!(\"{answer}\");\n```\n";
fn view(_: &()) -> El<()> {
    #[cfg(feature = "markdown")]
    let document = stillkit_kit::Markdown::new(DOCUMENT)
        .on_link(|_, url| println!("App handles {url}"))
        .build();
    #[cfg(not(feature = "markdown"))]
    let document = el::text(DOCUMENT).selectable();
    el::col()
        .h_full()
        .pad(24.0)
        .fill(el::token::SURFACE)
        .scroll(el::Axis::Vertical)
        .child(document)
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Markdown viewer",
        size: (640.0, 760.0),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: (),
        view,
    })
}
