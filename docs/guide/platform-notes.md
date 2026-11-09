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

## Opt-in desktop services

`tray::Tray::new` creates an icon from validated straight RGBA pixels. Keep it on the
UI thread, drain `next_event`, and sleep with `platform::current::host::wait`.
Creation never shows or activates a window. Drop removes the icon. The `services`
example is a tray-only app; right-click exits it.

Windows resolves shell APIs on first use and restores icons after Explorer restarts.
Linux uses StatusNotifierItem on the session bus, sharing the AT-SPI wire reader and
joining the Wayland poll set. A StatusNotifier host is required (GNOME needs an extension).
Tooltip changes signal the host. Primary and context-menu requests return screen
coordinates; `Tray::open_menu` opens stillkit's own `Menu` there. The app routes menu
surface events to the menu as usual. Wayland compositors control global placement;
tray-menu placement is therefore compositor-dependent. macOS currently reports an
unsupported NSStatusItem stub.

`hotkey::Hotkey::new("CTRL+ALT+K".parse()?, "Open panel")` registers a global chord.
Drain `activated()` after the wait. Drop unregisters it. Linux uses GlobalShortcuts,
requires compositor support and may show a consent dialog (up to two minutes).
The compositor chooses the actual trigger. macOS hotkeys are currently unsupported.

`notify::Notifications::new(id, icon)` owns desktop notifications. `show(title, body)`
returns an id; `next_action` reports the id and action key after dispatch. Windows
uses tray balloons (one active balloon at a time, default action, shell text limits), avoiding
WinRT startup and registration. Linux uses org.freedesktop.Notifications; action
support depends on the notification server. Drop closes owned notifications.

Call `instance::Instance::claim(id, std::env::args_os().collect())` before creating UI.
`Some(guard)` is the first launch; `None` means arguments were acknowledged by the
first launch and this process should return from main. Drain `next_args()` after
native dispatch. URL activations are arguments; register URL schemes in your app's
installer or desktop file. Windows uses a session-local named mutex and a hidden
WM_COPYDATA endpoint; Linux uses a D-Bus name. Forwarding is bounded to 64 KiB and
five seconds. Native argument encodings are preserved. Drop releases ownership.
