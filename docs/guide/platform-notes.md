# Platform notes

Windows has the full backend: CPU/GPU surfaces, native editing and UI Automation through AccessKit. Set Identity once before any window if other processes discover your windows by class name. Use your own prefix; do not copy mypaper class names. Surface windows use the class <prefix>, app windows <prefix>.app, and desktop-band surfaces carry the <prefix>.desktop window property. desktop_classes lists other window classes that share the desktop band; surfaces keep their order relative to them.

Linux supports Wayland CPU and GPU; no X11 or AT-SPI yet. Normal windows use xdg-shell. Bars need wlr-layer-shell (KDE/wlroots). GNOME has no layer-shell: a real bar there requires a Shell extension. Headless CPU tests need fonts/fontconfig, not a compositor. Clipboard and IME are implemented but need broader live compositor testing.

macOS backend is in progress and untested. Cross-compilation checks types, not native behavior.

```rust
use stillkit::el::{self, App, El, col, text};
use stillkit::{Kind, Scheme, Theme};
type State = ();
fn view(s: &State) -> El<State> {
    let _ = s;
    col().pad(24.).child(text("One view, native hosts"))
}
fn main() -> anyhow::Result<()> {
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
```

[Executable source](../../crates/stillkit/examples/guide_platform_notes.rs).


[All guides](../../README.md#guides) ? [Element reference](../el.md)
