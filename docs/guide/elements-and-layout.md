# Elements and layout

Compose col, row and grid(n). Layout lengths are design pixels; theme tokens follow the chosen look. grow shares spare space, explicit widths keep a sidebar steady. Give scroll views a bounded height or grow inside a bounded parent.

Keys retain identity when siblings reorder; keep them stable and unique among siblings. fill, color, radius, opacity and shadow style an element. label, role, checked, selected and value supply semantics. The same tree works with either renderer.

```rust
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
```

[Executable source](../../crates/stillkit/examples/guide_elements_and_layout.rs).


[All guides](../../README.md#guides) | [Element reference](../el.md)
