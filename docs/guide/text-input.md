# Text input

With value supplied, the view owns the string. on_input receives the complete edited text: give the old value back to reject an edit, or a transformed value to normalize it. Without value the field keeps its own text. A stable key retains selection and undo.

text_input is single-line and Enter calls on_submit. text_area wraps and scrolls; Ctrl+Enter submits. secure() masks passwords and withholds text from accessibility and tree dumps. max_len counts characters. label names the field for assistive technology. Windows uses RichEdit while focused, including IME, undo and clipboard; Wayland uses the runtime editor, text-input-v3 and seat clipboard. Linux native editing needs more live verification.

```rust
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
```

[Executable source](../../crates/stillkit/examples/guide_text_input.rs).


[All guides](../../README.md#guides) ? [Element reference](../el.md)
