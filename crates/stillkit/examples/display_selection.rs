//! Selectable help text copied as one document.
use stillkit::el::{self, App, Axis, El, col, text};
use stillkit::{Kind, Scheme, TextRole, Theme};
fn view(_: &()) -> El<()> {
    col().pad(20.0).gap(12.0).scroll(Axis::Vertical).selectable()
        .child(text("Help and About").font(TextRole::Title2))
        .child(text("Select text across paragraphs, then press Ctrl+C to copy it into another app."))
        .child(text("Double-click selects a word. Triple-click selects a paragraph. Shift-click extends the selection."))
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Document viewer",
        size: (560.0, 400.0),
        kind: Kind::App { min: (280, 200) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: (),
        view,
    })
}
