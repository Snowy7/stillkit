# stillkit

A native UI library that stays still: surfaces are drawn on the CPU, redraw only the rectangle
that changed, and do no work at all while nothing moves. No GPU device, ever.

Status: **pre-release, Windows only.** It was extracted from
[mypaper](https://github.com/Snowy7/desktop-wallpaper) (its shell and Studio are built on it), with
history. macOS and Linux backends, a declarative element tree, a shared animation runtime and one
component set are planned; see the plan linked below.

![The kit, Liquid style, dark](docs/media/native/kit-liquid-dark.png)

## What it is today

- **Surfaces**: per-pixel-alpha layered windows (popups, a bar, desktop-band windows) and normal
  app windows, drawn with software Direct2D and DirectWrite into a WIC bitmap.
- **A frame** is `view(&model, &theme) -> DisplayList`. `Surface::present` compares it with the list
  on screen and redraws only the bounds that changed; an identical frame costs one comparison.
- **The kit**: themes (Liquid, One UI, Solid, Fluent, Mono; dark and light), text roles, Lucide
  icons, buttons, switches, sliders, segmented controls, rows, tiles, fields, menus, scroll views,
  tooltips, charts and a calendar.
- **Accessibility** through AccessKit (UI Automation), built only once a client asks.
- **Golden images**: headless renders through the same software target as the screen.

Measured (a 3840x36 bar): 8.5 MB private, 0 wakeups over 95 s idle, a whole-bar redraw in 0.33 ms
(median). Details in [docs/ui.md](docs/ui.md).

## Use it

```toml
[dependencies]
stillkit = { git = "https://github.com/Snowy7/stillkit", rev = "<rev>" }

# Cargo applies only the root workspace's patches, so apps repeat this one:
[patch.crates-io]
accesskit_windows = { git = "https://github.com/Snowy7/stillkit", rev = "<rev>" }
```

An app whose other processes find its windows by class name sets its own names once, before the
first window:

```rust
stillkit::set_identity(stillkit::Identity { prefix: "myapp.ui", desktop_classes: &[] })
    .expect("before the first window");
```

## Develop

```sh
cargo test -p stillkit --features golden
cargo run --release -p stillkit --example gallery     # writes docs/media/native/kit-*.png
STILLKIT_BLESS=1 cargo test                            # rewrites golden PNGs
```

The toolchain is nightly (Cranelift for fast dev builds, see `.cargo/config.toml`).

MIT.
