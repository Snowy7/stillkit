# stillkit: rules for agents

stillkit is a native UI library, extracted from Glaze (`D:\Coding\projects\desktop\wallpaper`, whose
shell, Studio and Files are built on it). The plan, with phases and decisions, is
`plans/ui-library-extraction.html` (published at https://faca9c667eb799c7.snist.dev).

## Product rules (never break these)

1. **CPU mode never loads a GPU driver, on any OS.** CPU is the default (software Direct2D on a
   WIC bitmap; vello_cpu on Linux). Vendor drivers (`nv*umx`, `nvgpucomp64`, `amdxc64`, `amdxx64`,
   `atidx*`; Metal devices; libEGL, libGL, libvulkan) load only when an app or surface asks for
   GPU mode (`Render::Gpu`, the `gpu` feature), and GPU code isn't even compiled without that
   feature. `tests/no_gpu_driver.rs` checks CPU mode, `tests/gpu_driver.rs` GPU mode. A GPU
   device costs 30–250 MB (docs/ui.md, "Render modes"), so take it only for glass or heavy
   animation, and keep small surfaces on the CPU beside it.
5. **Every mode works, and Glaze never pays for it.** CPU alone, GPU alone, and both in one
   process (per surface). Glaze runs CPU mode; its numbers must not get worse.
2. **Nothing changes, nothing is drawn.** A frame is compared with the one on screen and only the
   changed bounds are redrawn; an identical frame costs one comparison.
3. **Zero idle work.** No timers or tickers while nothing moves; the frame ticker runs only while
   something animates.
4. **RAM counts as much as CPU.** Measure before and after anything that could grow memory.

## Code

- Rust 2024, nightly (Cranelift dev builds, `.cargo/config.toml`). Typesafe, inferred types, no
  needless wrappers. Keep it simple; build only what a phase needs.
- Short doc comments on public items say how they are used. Keep comments in sync with changes.
- Tests are focused: behaviour and goldens, not smoke tests.
- `Glaze` uses this crate as `glaze_ui` (a renamed dependency) at about 1,000 call sites. Don't
  break public paths without a compatibility re-export, unless your brief says otherwise.

## Commands

```sh
cargo check -p stillkit --all-targets --features golden
cargo test -p stillkit --features golden
cargo clippy -p stillkit --all-targets --features golden
cargo run --release -p stillkit --example gallery      # docs/media/native/kit-*.png
STILLKIT_BLESS=1 cargo test -p stillkit --features golden   # rewrite goldens, then look at them
```

Use your own `CARGO_TARGET_DIR` under `D:\stillkit-targets\` when you work beside others (never C:).
Don't run the Glaze app or deploy it. Glaze builds this checkout through a local `[patch]`
(its `.cargo/config.toml`), so a broken commit here breaks Glaze's next build.

## Git

- Commit with clear messages (`feat(el): …`, `refactor(platform): …`). Commit only your own paths.
- Never rewrite `main`'s history, and never force-push.
