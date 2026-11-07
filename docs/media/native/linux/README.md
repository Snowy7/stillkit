# Linux native gallery and evidence

Generated on m6u, Ubuntu 26.04 / GNOME 50.1 Wayland / Ryzen 5 7640HS,
2026-11-07. The twelve `kit-*.png` pages use the same gallery code as Windows.
Opened every page in the contact sheet, and compared Liquid Dark directly with
`../kit-liquid-dark.png`. Titles, buttons, clipping, rings, translucent surfaces,
icons and menus are legible, without missing legacy menu glyphs.

Differences from Windows:

- Inter variable replaces Segoe UI Variable. Grayscale Vello glyph coverage,
  baselines, advance widths and wrapping differ slightly from DirectWrite.
  Weight 600 uses normalized font coordinates; a focused test verifies its ink
  coverage lies between 400 and 700. Tracking is converted from pixels to em.
- The extracted repo has no `themes/rain-city/preview.jpg`. On Linux the gallery
  uses a deterministic blue gradient fallback, so glass samples have no wallpaper
  texture. Other platforms retain their existing wallpaper behavior.
- Windows-only private-use icon fonts are replaced by bundled Lucide paths.
  Thus the icon sheet's “Segoe” rows show the vector fallback, not a font specimen.
- Text-quality lab rows intentionally look identical: DirectWrite's gamma,
  contrast, grid-fit and rendering-mode controls have no Linux equivalent here.
- Shadow compositing and antialiased clip edges have small raster differences.

`hello.png` is a headless export of the same display list as the live hello, not
a desktop screenshot. GNOME's screenshot D-Bus API returned `AccessDenied:
Screenshot is not allowed`; the available desktop capture returned black.
The wire trace confirms the real surface was configured, committed, and entered
the output. No window was activated, and no mouse/keyboard input was synthesized.

## Measurements

`measurement.json`: final release hello, 400×220, real `/run/user/1000/wayland-0`.
Private = Private_Clean + Private_Dirty from smaps_rollup, not RSS minus libraries.
After 3 seconds of startup settling, over a 30-second interval:

- Private: **6,016 → 6,016 KiB**; PSS **6,382 KiB**; RSS **9,016 KiB**.
- User/system CPU: **0 / 0 ticks**, at 100 Hz accounting resolution.
- Voluntary/involuntary context-switch deltas: **0 / 0**.
- Forbidden GPU libraries: **0**, with mappings in `maps.txt`.
- `protocol.log`: one content attach, one physical `damage_buffer(0,0,400,220)`,
  no frame requests and no activated configure state. Startup protocol messages
  are excluded from the steady-idle counter claim.

The spike's empty native window was 1,396 KiB private at end; this hello is
4,620 KiB above that earlier baseline (different executable, not a paired trial).
`measurement-before-font-sharing.json` records the 8,452 KiB intermediate result
with copied eager regular+italic fonts. Static font bytes and lazy italic loading
avoid that extra retained memory. Variable outlines increase resource preparation
slightly; the final number above includes real 600-weight text.

## Reproduce

From the repository root, with Cargo on PATH:

```sh
cargo test -p stillkit --features golden
cargo clippy -p stillkit --all-targets --features golden
cargo check -p stillkit --all-targets --features golden --target x86_64-pc-windows-msvc
cargo run --release -p stillkit --example gallery -- /tmp/stillkit-gallery
cargo build --release -p stillkit --features golden --example linux_hello
python3 docs/media/native/linux/measure.py
# Optional callback-driven animation (no idle ticker afterwards):
cargo run --release -p stillkit --example linux_hello -- --animate
```

The measurement script temporarily changes GNOME's `focus-new-windows` to `strict`
and restores it in `finally`; do not run overlapping instances. Evidence goes to
`/tmp/stillkit-live`. It terminates its own client. `--snapshot=/tmp/hello.png` is
available when built with `golden`, and is excluded from the final memory run.

Linux tests and Clippy succeed, with eight existing dead-code warnings in the
Windows accessibility glue (`src/a11y.rs`). Windows cross-target checking succeeds.
No Windows implementation or core contracts were modified.

The optional 600 ms animation was observed for four seconds: **33 frame requests**
over **585 ms**, then no further requests/commits. After the initial full frame,
damage was the ring's **38×38 physical pixels**, rather than the whole window.
`animation.log` and `animation.json` retain the trace and summary. Full suite:
**56 unit tests + 1 Linux GPU-boundary integration test passed**, with one ignored
documentation example and the Windows-only integration tests gated out on Linux.
