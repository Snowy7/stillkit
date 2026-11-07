# stillkit Linux CPU rendering spike

> Copied from m6u:/home/islam/projects/stillkit-spike-linux (its git repo holds the code, raw logs and traces).

Measured on m6u, 2026-11-07: Ubuntu 26.04.1, GNOME Shell/Mutter 50.1, Wayland, Ryzen 5 7640HS / Radeon 760M. Real `/run/user/1000/wayland-0`; no nested compositor. Release Rust builds, single rendering thread. No system packages installed; Cargo fetched the locked Rust dependencies. All work and artifacts are in this repository; no remote.

## 1. Numbers

| Configuration | Private 10s / end (KiB) | PSS 10s / end | RSS 10s / end | Launch → commit (ms) | Stripped bytes | Rust deps |
|---|---:|---:|---:|---:|---:|---:|
| Blank baseline 1× | 1,400 / 1,396 | 1,792 / 1,801 | 3,988 / 3,984 | 6.753 | 706,688 | 28 |
| A 1× | 5,104 / 6,280 | 5,633 / 6,809 | 8,332 / 9,508 | 27.020 | 3,756,704 | 62 |
| B native outlines 1× | 6,644 / 7,396 | 7,215 / 7,967 | 10,152 / 10,904 | 24.936 | 4,348,928 | 73 |
| B bitmap atlas 1× | 72,488 / 138,564 | 73,064 / 139,140 | 76,032 / 142,108 | 51.129 | 4,348,928 | 73 |
| A 2× | 8,720 / 13,444 | 10,129 / 14,853 | 13,692 / 18,416 | 41.227 | 3,756,704 | 62 |
| B native outlines 2× | 12,200 / 16,908 | 13,665 / 18,373 | 17,524 / 22,232 | 27.792 | 4,348,928 | 73 |
| B bitmap atlas 2× | 77,892 / 145,772 | 79,324 / 147,229 | 83,156 / 151,036 | 59.645 | 4,348,928 | 73 |

Rust dependency counts are unique name/version pairs in the normal dependency graph, excluding this package and build/dev-only edges. Dynamic library counts excluding loader/vDSO: baseline 2, A 3, B 3. Both B modes use the same executable. All four mapping samples for every row have **zero forbidden GPU libraries**.

| Renderer | Full median / p95 (µs) | Label median / p95 (µs) |
|---|---:|---:|
| A 1× | 4662.622 / 4711.603 | 20.528 / 20.648 |
| B native outlines 1× | 523.995 / 530.558 | 30.866 / 31.368 |
| B bitmap atlas 1× | 486.987 / 490.875 | 22.251 / 22.942 |
| A 2× | 16805.409 / 16979.210 | 73.145 / 77.754 |
| B native outlines 2× | 1143.196 / 1165.408 | 45.323 / 46.807 |
| B bitmap atlas 2× | 1127.117 / 1172.030 | 34.183 / 35.676 |

| Configuration | Whole idle 1 Δutime / Δstime / Δvoluntary | Whole idle 2 Δutime / Δstime / Δvoluntary | Steady idle 1 ΔCPU ticks / Δvoluntary | Steady idle 2 ΔCPU ticks / Δvoluntary |
|---|---:|---:|---:|---:|
| Blank baseline 1× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |
| A 1× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |
| B native outlines 1× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |
| B bitmap atlas 1× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |
| A 2× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |
| B native outlines 2× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |
| B bitmap atlas 2× | 0 / 0 / 3 | 0 / 0 / 1 | 0 / 0 | 0 / 0 |

The older full-run logs label their animation-update counter `frame_callbacks`; it includes the initial animation kick and is not an exact count of callback events (the blank baseline's value of 1 is that kick, not a callback request). The final client records animation updates and received callback events separately. See the separate wire trace for actual requests.

Private means **Private_Clean + Private_Dirty**, not RSS minus shared libraries. Units in memory tables are KiB, binary sizes are bytes. The end sample is at 71.8 seconds, just before normal exit at 72 seconds; the first is 10 seconds after the first content commit. Raw rollup values and GPU library lists are in `results/*-memory.json`; the corresponding full mapping lists are `results/*-*.maps`.

Draw benchmarks use 20 warmup draws and 200 timed draws per case, with median index 100 and p95 index 189 after sorting. Both include command generation, cached text layout/glyph lookup, rasterization, and returning an owned RGBA bitmap. Label draws rasterize only a 190×26 logical-pixel patch. Full scene draws include the layered soft shadow. Timings exclude Wayland presentation, RGBA→native ARGB conversion, and the copy into shm. Thus they are renderer timings, not complete update latency. Separate invocations produce the 1× and 2× measurements; no cold-cache or repeat-run confidence interval is claimed.

The table's “Launch → commit” uses separate fresh `--startup-only` invocations: the parent timestamps immediately before process creation, and subtracts that from the child's wall-clock timestamp just after flushing its first content commit. This includes subprocess launch overhead, exec/ELF loading, fontconfig discovery, font loading, Wayland setup and rasterization. It slightly overestimates process-start-to-commit by including the parent's spawn overhead; it is not a presentation/scanout timestamp. `results/startup.json` retains these readings. The main script logs additionally report main-entry-to-commit times. All startup numbers are single warm-filesystem observations; the two clocks are assumed not to step during these short runs.

### Idle interpretation

`results/*.log` records whole-phase deltas from `/proc/self/stat` fields utime/stime and `/proc/self/status` voluntary context switches. The clock tick rate is 100 Hz. A zero tick delta means less than the accounting resolution, not proof of zero instructions. Phase one begins immediately after the first commit and therefore includes startup configuration/buffer events. A voluntary switch counts **going to sleep**, too; it is not itself a hardware wakeup counter.

The external sampler also measures steady portions: 10–29.8 seconds and 42.2–71.8 seconds. It reads `/proc` without waking the client. These intentionally exclude startup protocol traffic and phase-boundary work. The whole 30-second raw context-switch deltas are preserved, rather than relabeled as zero. **The literal requirement of zero voluntary-switch delta over the complete first 30 seconds is not met.** No idle repaint or periodic frame/timer loop is implemented; an absolute promise of no wakeups from compositor pings, reconfiguration, or unrelated system events would be false.

The separate `WAYLAND_DEBUG=client` wire trace completed the same 72-second sequence with B's native outline path. [Parsed evidence](results/protocol-summary.json): **130 content commits**, exactly one full-scene damage, **119 slider-only damages/frame requests** between 30.033 and 31.995 seconds, and **10 label-only damages**. There were no content commits in either steady-idle interior and no activated=true state. The raw trace is [trace-b.stderr](results/trace-b.stderr); `verify_trace.py` checks the rectangles, timing and all captured GPU mapping lists. This diagnostic run is excluded from the renderer performance tables.

### GPU boundary

All forbidden-library matches are checked case-insensitively against `libEGL`, `libGL`, `libvulkan`, `libgbm`, `radeonsi`, and `libdrm_amdgpu`. `ldd` lists only libc, libgcc_s and the loader for the baseline, adding libm for A and B. Additional headless syscall traces (`results/gpu-a.trace`, `results/gpu-b.trace`) cover openat/mmap/ioctl including the fontconfig children; no GPU library or `/dev/dri` access was found. Window-run mapping snapshots are the direct `/proc` evidence. Snapshots cannot independently rule out a transient load between samples, but this program has no GPU backend, loader, device-creation API, or GPU dependency in either feature build. GNOME's existing compositor may use its GPU; that is outside the client's process budget.

## 2. Recommendation

Choose **B: vello_cpu 0.3.0 with native glyph outlines and the bitmap atlas disabled**, plus **cosmic-text + fontconfig** for shaping/font selection. Keep Vello's outline/hint preparation caches through persistent Resources. In this spike that mode retains almost all of Vello's full-scene speed advantage without the atlas memory cliff, at a modest private-memory premium over A. Run `results/renderer-b --no-atlas` (or `--no-atlas --scale 2`); the comparison executable deliberately keeps atlas-on as its default so that both configurations remain reproducible.

A (tiny-skia + cosmic-text + Swash) is a credible simpler fallback when minimum memory and dependency count dominate. Its one-label redraw is excellent. However, its uncached full-scene shadow rendering at 2× crosses a 60 Hz frame budget: the measured 2-second animation produced only 60 frame-driven updates, versus approximately 120 for B. Cache the static shadow before drawing a broad conclusion about all tiny-skia applications; this recommendation is for the actual measured implementation and scene.

Do **not** enable Vello's stock bitmap atlas for this small UI. A 4096×4096 RGBA page is 64 MiB; this spike's independent full-scene and patch contexts each eventually allocate one. The measured atlas-on private memory grows from about 71 MiB to 135 MiB at 1× when the label context first draws. The [released source fixes the default page size at 4096](https://docs.rs/vello_cpu/0.3.0/src/vello_cpu/render.rs.html), with no public Resources page-size setter in that release. This is a configuration-specific finding, not a claim that Vello CPU inherently needs 135 MiB. Turning off the bitmap atlas leaves its native glyph path and preparation caches in use.

The CPU-only renderer/window path is viable, but **the entire requested acceptance suite is not an unconditional pass**: literal whole-phase zero-switch counts, true semibold specimens, fractional/live scale handling, and production IME/accessibility integration have the qualifications below.

Use **raw wayland-client + xdg-shell + double-buffered ARGB8888 wl_shm** for ordinary GNOME windows. Keep a canonical CPU image, attach only released buffers, and send `damage_buffer` in physical pixels. In this implementation both shm buffers are refreshed from the canonical image when used; that deliberately simple correctness strategy copies a whole bitmap even for label updates. A production implementation can track per-buffer damage history to avoid that copy. The slider damages only `[34,267,292,26]`; one-second updates damage only `[40,345,190,26]`. Nothing renders or commits during either idle phase. Frame callbacks are requested only with animation commits; the baseline commits its blank bitmap once.

Text shaping is **cosmic-text 0.19 / HarfRust**, with fontconfig (`fc-match`) selecting the installed Noto Sans faces. A uses **Swash**'s glyph image cache; B shares the same shaped glyph positions but uses **vello_cpu's native glifo/skrifa glyph path**, not Swash pixels. Both native outline rendering (`--no-atlas`) and opt-in bitmap-atlas caching are measured. Both cache layouts. Vello render contexts, output pixmaps and per-context Resources persist across draws; no worker threads are enabled. Keeping one Resources instance per context follows [Vello's ownership guidance](https://docs.rs/vello_cpu/0.3.0/vello_cpu/struct.Resources.html).

### Text and visual checks

Verified by opening all six exported images and checking label sizing, clipping, rounded cards, alpha, icons, slider and switch:

| Renderer | 1× | 2× |
|---|---|---|
| A: tiny-skia + Swash | [360×420 PNG](a-1x.png) | [720×840 PNG](a-2x.png) |
| B: vello_cpu + native glyphs, atlas | [360×420 PNG](results/b-1x.png) | [720×840 PNG](results/b-2x.png) |

Both are legible at 11/13/14/17/22 logical pixels. Swash and Vello have small glyph coverage/positioning differences; neither shows missing glyphs in this Latin scene. The no-atlas specimens are also committed: [1×](b-no-atlas-1x.png), [2×](b-no-atlas-2x.png). These are grayscale-alpha text renderings, appropriate for a translucent surface; no LCD subpixel antialiasing claim is made. This is a visual inspection, not a multilingual shaping, screen-reader or IME test.

The focused `label_patch_matches_full_scene` test checks the full-vs-patch pixels at both scales, including translucent backdrop restoration. A matched exactly during development. Vello's u8 compositing rounds some glyph channels by one LSB after backdrop flattening; the final check allows at most one LSB per channel for both backends. Both release test configurations pass, including B with and without the bitmap atlas.

### Limits of the spike

- The scene requests weight 600, but this machine has Noto Sans Regular and Bold, not a real Noto Sans Semibold. Fontconfig resolves the semibold request to **Bold**. This is a concrete deviation from the requested true semibold specimen; deploy a variable font or actual 600 face before choosing final typography.
- The soft shadow uses twelve low-alpha rounded silhouettes, not a Gaussian blur kernel. Tiny-skia's full-scene result includes repainting all twelve; static shadow caching is an obvious next optimization and could change the renderer tradeoff. No cached-shadow result was measured.
- Integer output scale is used at startup, and both backends ran actual Wayland windows with forced 2× buffers on this host's 1× output. A physical HiDPI display was not tested. Fractional scaling and live monitor/scale changes are not implemented. The startup fallback takes the maximum advertised integer output scale until a preferred-buffer-scale event overrides it; mixed-monitor selection needs proper enter/leave tracking in production.
- The window is display-only: empty input region, no seat/pointer/keyboard bindings. To obey “do not take focus,” the measurement driver temporarily sets GNOME's `focus-new-windows` policy from `smart` to `strict`, restores it in `finally`, and records toplevel activated states. No activation request, synthetic keyboard input or mouse movement is used. This is a measurement accommodation, not a portable no-focus toplevel API.
- Renderer memory includes font/layout/glyph caches, retained full/patch render contexts and canonical image. No IME or AT-SPI adapter is linked; their future idle and memory costs are unmeasured. Linux private/PSS accounting and Windows private bytes are not equivalent. Shm pages shared with the compositor are visible in PSS, and compositor-side costs are not included.

## 3. Desktop integration research

### Bars and overlays

**GNOME:** the tested registry exposes no `zwlr_layer_shell_v1`; Mutter 50.1's [Wayland initialization](https://github.com/GNOME/mutter/blob/50.1/src/wayland/meta-wayland.c) implements no layer-shell path. An ordinary xdg toplevel can show a control panel, but cannot request desktop-edge anchoring, a reserved work area, always-on-top layering, or no keyboard focus. An xdg popup is parent-relative and temporary, not an independent desktop bar. Use a **GNOME Shell extension** for genuine panel/overlay integration: Shell provides [`Panel.addToStatusArea`](https://github.com/GNOME/gnome-shell/blob/50.1/js/ui/panel.js) and [`LayoutManager.addChrome` with `affectsStruts`](https://github.com/GNOME/gnome-shell/blob/50.1/js/ui/layout.js). Those are in-process Shell APIs, not a public protocol that embeds an arbitrary external wl_shm surface. A companion extension plus IPC is an architectural option, not something demonstrated by this spike; it has version compatibility and host-process cost implications.

**KDE and wlroots compositors:** use wlr-layer-shell when advertised. The [protocol](https://github.com/swaywm/wlr-protocols/blob/master/unstable/wlr-layer-shell-unstable-v1.xml) specifies layers, edge anchors, margins, exclusive zones to reserve space, and keyboard interactivity (including none). KDE [KWin implements it](https://github.com/KDE/kwin/blob/master/src/wayland/layershell_v1.cpp); wlroots support still depends on the compositor enabling the protocol. This surface role replaces xdg-toplevel; it is not an extra flag to apply to an existing toplevel.

**X11:** an override-redirect window bypasses normal window-manager management; it can be placed directly but takes responsibility for its own behavior. A managed desktop bar should instead advertise `_NET_WM_WINDOW_TYPE_DOCK`, and reserve its edge with `_NET_WM_STRUT_PARTIAL` (or legacy `_NET_WM_STRUT`). The [EWMH specification](https://specifications.freedesktop.org/wm-spec/latest/ar01s05.html) describes the dock type and strut coordinates. [Xlib window attributes](https://www.x.org/releases/current/doc/libX11/libX11/libX11.html) define override-redirect. Do not assume an Xwayland dock/strut yields the same native Wayland work-area semantics; an X11 backend is a distinct compatibility path, not a GNOME Wayland bar solution established here.

### IME

Both Mutter and KWin support **text-input-v3**. This session advertises `zwp_text_input_manager_v3` version **1** (the `_v3` name is the protocol generation, not that object's version). [Mutter 50.1 implements it](https://github.com/GNOME/mutter/blob/50.1/src/wayland/meta-wayland-text-input.c), with its [version constant](https://github.com/GNOME/mutter/blob/50.1/src/wayland/meta-wayland-versions.h). [Current KWin source](https://github.com/KDE/kwin/blob/master/src/wayland/textinput_v3.cpp) supports object version 2. Negotiate the advertised object version; implement enter/leave, enable/disable, surrounding text with UTF-8 byte offsets, cursor rectangle, content purpose/hints, preedit, commit, deletion and done batching. This spike does not implement editing or test an input method.

An **IBus client over D-Bus** is a fallback when a useful Wayland text-input integration is unavailable: create an input context, feed key events, and handle preedit/commit/forwarded keys plus focus and surrounding text. The [IBus input-context API](https://github.com/ibus/ibus/blob/main/src/ibusinputcontext.h) documents these operations. It is an additional backend dependent on an installed/running IBus service, not a universal Wayland replacement; candidate placement and desktop integration need separate validation. GNOME's compositor-side text-input bridge is preferable to requiring every native client to bind directly to IBus.

### Accessibility

AccessKit has a shipped **Unix adapter exposing its tree over AT-SPI**; this is not merely a proposal. The [upstream README](https://github.com/AccessKit/accesskit/blob/main/platforms/unix/README.md) describes the bridge and its internal async tasks despite a blocking public API; the [published Rust adapter documentation](https://docs.rs/accesskit_unix/latest/accesskit_unix/) is available. A raw Wayland application must create and update its accessibility tree, route actions, and provide focus/bounds semantics; renderer choice does not supply that automatically. The standalone Unix adapter is the relevant route without adopting winit. Its executor/D-Bus activity needs an idle-budget measurement with an actual screen reader; this spike cannot claim accessible controls or an unchanged zero-idle budget after adding it.

### Translucency and blur

Alpha in ARGB8888 wl_shm works independently of any blur extension. KDE's [`org_kde_kwin_blur_manager` / `org_kde_kwin_blur`](https://github.com/KDE/kwin/blob/Plasma/6.5/src/wayland/blur.cpp) lets a client designate the surface/region whose background should be blurred; KWin's compositor/effect performs the blur, not the client's CPU renderer. Discover it at runtime and tolerate its absence or disabled effect.

**GNOME needs a version-qualified answer.** This tested GNOME 50.1 session advertises neither KDE blur nor a background-effect global. However, [current upstream Mutter implements `ext-background-effect-v1`](https://github.com/GNOME/mutter/blob/main/src/wayland/meta-wayland-background-effect.c), including a blur-region request; its [current version definitions](https://github.com/GNOME/mutter/blob/main/src/wayland/meta-wayland-versions.h) include that protocol. Therefore “GNOME has no blur protocol” is no longer a safe blanket statement. Detect `ext_background_effect_manager_v1` on newer deployments and fall back to plain translucency on this host. Older GNOME solutions require compositor-side Shell extensions; no CPU-only client can blur pixels behind itself without obtaining those pixels or compositor cooperation. No blur-behind implementation or performance result is claimed here.

## 4. What surprised me

1. **A CPU glyph cache can recreate the GPU-sized memory problem.** Vello's optional atlas, rather than a GPU driver, was responsible for the 64 MiB steps. Disabling just that cache preserved most of its speed and changed the recommendation.
2. **The soft shadow dominates the naive tiny-skia full draw.** At 2× the scene drops to roughly half the available frame callbacks, even though the label patch remains cheap. Retained static effects matter more here than text throughput.
3. **“Zero wakeups” and voluntary context-switch deltas need different interpretations.** Every complete idle interval includes a switch into sleep, and the first also includes protocol setup. The steady measurements distinguish that from a polling/render loop.
4. **The GNOME blur answer has changed upstream.** This host still lacks a client blur protocol, while current Mutter has a standard background-effect implementation. Runtime capability detection is necessary.

## Reproduce

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --no-default-features
cp target/release/stillkit-spike results/baseline
cargo build --release
cp target/release/stillkit-spike results/renderer-a
cargo build --release --features vello
cp target/release/stillkit-spike results/renderer-b
python3 measure.py
python3 measure.py --no-atlas
python3 startup.py
python3 measure.py --trace
python3 verify_trace.py
python3 summarize.py
results/renderer-a --headless results/a-1x.png
results/renderer-a --headless results/a-2x.png --scale 2
results/renderer-b --headless results/b-1x.png
results/renderer-b --headless results/b-2x.png --scale 2
results/renderer-b --headless results/b-no-atlas-1x.png --no-atlas
results/renderer-b --headless results/b-no-atlas-2x.png --no-atlas --scale 2
results/renderer-a --bench
results/renderer-b --bench --scale 2
results/renderer-b --bench --no-atlas
results/renderer-b --bench --no-atlas --scale 2
cargo test --release
cargo test --release --features vello
```

The default binary is A; `--features vello` selects B behind the same small `Renderer` trait at compile time, keeping the unused renderer out of each executable. The separate no-default-features build is a blank client: tiny-skia is retained as pixel storage/optional PNG utility, but it includes no font/shaping dependencies or scene renderer. Binary artifacts, dependency trees, build/test logs, PNGs and measurement evidence are committed under `results/`. All measurement and tracing clients exited normally, and the original `smart` focus policy was verified restored. No compositor, dev server or background service was started or left running.
