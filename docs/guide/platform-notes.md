# Platform notes

Windows has the full backend: CPU/GPU surfaces, native editing and UI Automation through AccessKit. Set Identity once before any window if other processes discover your windows by class name. Use your own prefix; do not copy mypaper class names. Surface windows use the class <prefix>, app windows <prefix>.app, and desktop-band surfaces carry the <prefix>.desktop window property. desktop_classes lists other window classes that share the desktop band; surfaces keep their order relative to them.

Linux supports Wayland CPU and GPU; no X11 backend. AT-SPI uses the same AccessKit tree as Windows (compile-checked; not compositor-tested here). Normal windows use xdg-shell. Bars need wlr-layer-shell (KDE/wlroots). GNOME has no layer-shell: a real bar there requires a Shell extension. Headless CPU tests need fonts/fontconfig, not a compositor. Clipboard and IME are implemented but need broader live compositor testing.

Identity and set_identity have the same shape on every platform. Set them once before creating windows. Windows uses the classes above; Wayland uses prefix as app_id (there is no X11 backend or WM_CLASS yet). macOS accepts the identity but leaves AppKit and bundle names unchanged. desktop_classes is used only on Windows.

macOS implements AppKit windows, CoreGraphics/CoreText CPU drawing, native input and AccessKit; GPU/Metal is optional. It is compile-checked, with native behavior still unverified.

AT-SPI is opt-in at process startup: set `STILLKIT_ACCESSIBILITY=1` when launching with Orca or an inspector. AccessKit Unix 0.24 starts an executor thread and opens D-Bus discovery eagerly in `Adapter::new`; stillkit gates construction so the default path opens neither D-Bus nor an accessibility wake socket. Trees are built only when AccessKit requests activation. Actions wake the UI's blocking poll through a socket, with no periodic timer. Enabling accessibility permits AccessKit's D-Bus executor even without a running screen reader. Automatic screen-reader discovery without that connection is not implemented; restart with the setting enabled if a client starts later.

On m6u, verify Orca discovery on Wayland, late client activation with the setting enabled, names/roles/focus, ranges and text values, reorder actions, live status announcements, multi-window close and reconnect. Compare idle wakes with the setting unset and with Orca stopped; check CPU mode still loads no GPU driver.

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


[All guides](../../README.md#guides) | [Element reference](../el.md)
