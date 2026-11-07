# Linux native gallery and evidence

Generated on m6u, Ubuntu 26.04 / GNOME 50.1 Wayland / Ryzen 5 7640HS,
2026-11-07. The fourteen `kit-*.png` pages use the same gallery code as Windows.
Opened every page in the contact sheet, and compared Liquid Dark directly with
`../kit-liquid-dark.png`. Titles, buttons, clipping, rings, translucent surfaces,
icons and menus are legible, without missing legacy menu glyphs.

Differences from Windows:

- Inter variable replaces Segoe UI Variable. Grayscale Vello glyph coverage,
  baselines, advance widths and wrapping differ slightly from DirectWrite.
  Weight 600 uses normalized font coordinates; a focused test verifies its ink
  coverage lies between 400 and 700. Tracking is converted from pixels to em.
- The generated dusk backdrop now matches the gallery on every platform. The mix
  page retains its own diagnostic coloured backdrop. Gallery glass is a CPU stand-in;
  `gpu/glass.png` demonstrates actual in-process Vulkan optics.
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
python3 docs/media/native/linux/measure.py --example linux_hello --settle 3 --out /tmp/stillkit-live
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

## Element host (merged main)

`el-hello-{1,2}x.png` and `el-sound-{1,2}x.png` are headless exports, inspected at
both scales. The hello label and switch, and Sound's slider, volume icon, mute
label and switch render without clipping. `kit-mix-{light,dark}.png` are the
Linux gallery test references: cosmic-text/Vello differs from Windows text ink,
so the test selects these without changing the Windows references.

`--snap` and `--tree` were run with an invalid Wayland display. A `--script`
click on `switch` changed checked to true and the label to “The light is on”.
The real GNOME session mapped both hello (320×180) and sound (340×160); protocol
traces confirm buffer attachment, damage and output enter. No activation request
or synthetic pointer/keyboard input was sent. These PNGs remain client exports,
not compositor screenshots. Chrome actions retain the real-input serial handling:
caption move, minimize, maximize/unmaximize and close; interactive chrome and
physical mixed-DPI hardware have not been exercised in this pass.

Final release hello: after 10 s settling, over 30 s, **5,992 → 5,688 KiB private**,
RSS **9,056 → 8,752 KiB**, PSS **6,299 → 5,995 KiB**. Private is the sum of
Private_Clean and Private_Dirty in smaps_rollup. CPU deltas **0 / 0 ticks**
(100 Hz), context-switch deltas **0 / 0**, GPU-library mappings **0**.
Resident pages fell rather than growing. A separate earlier run before the final
geometry update held 5,964 KiB private; it is not a paired memory comparison.
`el-measurement.json`, `el-maps.txt` and `el-protocol.log` retain final evidence.
The trace contains one content attach/damage and zero frame requests or activated
configure states. The process slept on the Wayland fd, with no idle ticker.

Reproduce the current element evidence:

```sh
cargo build --release -p stillkit --example el_hello --example el_sound
python3 docs/media/native/linux/measure.py
# Defaults: --example el_hello --settle 10 --out /tmp/stillkit-el-live
for app in hello sound; do
  for scale in 1 2; do
    target/release/examples/el_$app --snap /tmp/el-$app-${scale}x.png --scale $scale
  done
done
cargo test -p stillkit --features golden,serde
cargo clippy -p stillkit --all-targets --features golden,serde
cargo check -p stillkit --all-targets --features golden,serde --target x86_64-pc-windows-msvc
```

All gates pass: 64 unit tests, 7 element-runtime tests, 2 Sound tests, 1 Linux
GPU-boundary test, 1 gallery golden test and 1 compiled doc example; 1 doc example
ignored. Clippy reports only the eight existing accessibility dead-code warnings.
The only source changes outside the Linux backend are `linux_hello`'s explicit
wait for the new nonblocking queue contract, and gallery's Linux reference path.
No `src/el/` or core runtime changes were needed.

## In-process Vulkan mode

The `gpu/` directory contains the four final release measurements (`measurements.json`,
individual JSON files, protocol logs and client-frame PNGs). The full table and contract
are in `docs/ui.md`, “Linux GPU mode”. Opened and inspected CPU, GPU and merged glass
frames, plus the regenerated gallery contact sheet and Linux motion strips.

Reproduce from the root (run measurement scripts sequentially):

```sh
cargo build --release -p stillkit --features golden --example linux_gpu_measure
cp target/release/examples/linux_gpu_measure /tmp/stillkit-cpu-measure
cargo build --release -p stillkit --features gpu,golden --example linux_gpu_measure
python3 docs/media/native/linux/measure_gpu.py --cpu-binary /tmp/stillkit-cpu-measure
```

The drag is scripted application state at 60 Hz, not pointer injection. Idle blocks on
Wayland and stdin; it has no polling timeout. The GPU feature adds a libwayland system
backend (dlopen) to supply native Vulkan handles; the feature-off CPU path keeps its
original pure-Rust Wayland client. A preliminary feature-enabled CPU-only trial used
6.83 MiB private versus 5.60 MiB in the final feature-off CPU trial; no GPU libraries
loaded in either. Normal CPU applications retain their original dependency boundary.

The explicit live integration test is ignored by default because it needs a Wayland
session with strict focus policy: `cargo test -p stillkit --features gpu --test
linux_gpu_native -- --ignored`. It exercises mixed surfaces, retained partial frames,
resize, hide/show, opacity and a client-owned glass backdrop.
