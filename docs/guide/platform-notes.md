# Platform notes

Windows has the full backend: CPU/GPU surfaces, native editing and UI Automation through AccessKit. Set Identity once before any window if other processes discover your windows by class name. Use your own prefix; do not copy Glaze class names. Surface windows use the class <prefix>, app windows <prefix>.app, and desktop-band surfaces carry the <prefix>.desktop window property. desktop_classes lists other window classes that share the desktop band; surfaces keep their order relative to them.

Linux supports Wayland CPU and GPU; no X11 backend. AT-SPI uses the same AccessKit tree as Windows and turns itself on when a screen reader runs (compile-checked; not compositor-tested here). Normal windows use xdg-shell. Bars need wlr-layer-shell (KDE/wlroots). GNOME has no layer-shell: a real bar there requires a Shell extension. Headless CPU tests need fonts/fontconfig, not a compositor. Clipboard and IME are implemented but need broader live compositor testing.

Identity and set_identity have the same shape on every platform. Set them once before creating windows. Windows uses the classes above; Wayland uses prefix as app_id (there is no X11 backend or WM_CLASS yet). macOS accepts the identity but leaves AppKit and bundle names unchanged. desktop_classes is used only on Windows.

macOS implements AppKit windows, CoreGraphics/CoreText CPU drawing, native input and AccessKit; GPU/Metal is optional. It is compile-checked, with native behavior still unverified.

AT-SPI is on by default and costs one idle socket until assistive technology is running. Each UI thread asks the session bus once, without blocking, for `org.a11y.Status` (`IsEnabled || ScreenReaderEnabled`, as Qt reads it) and subscribes to its `PropertiesChanged`, on one socket that waits in the Wayland poll beside the display fd. Windows get AccessKit adapters only while that says on: starting Orca brings them up with the latest tree, and switching it off drops them. AccessKit Unix 0.24 cannot stop its executor, so after the first adapter its two threads (the executor and `async-io`) and their session-bus connection stay for the rest of the process. They are blocked on sockets and wake only for D-Bus traffic. With no session bus, or no running `org.a11y.Bus` (stillkit never starts it), nothing is opened. `STILLKIT_ACCESSIBILITY=1` forces adapters on and `0` forces them off, with no socket; unset means automatic. Trees are built only when AccessKit requests activation, and actions wake the poll through a socket. There is no timer. docs/ui.md ("Linux accessibility") gives the design and the m6u checklist.

On m6u, run the checklist in docs/ui.md ("Linux accessibility"): fds and threads in each state, Orca start and stop, idle wakes, the overrides. Then check names, roles and focus, ranges and text values, reorder actions, live status announcements, multi-window close and reconnect, and that CPU mode still loads no GPU driver.

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
