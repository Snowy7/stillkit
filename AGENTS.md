# stillkit: rules for agents

stillkit is a native UI library, extracted from mypaper (`D:\Coding\projects\desktop\wallpaper`, whose
shell, Studio and Files are built on it). The plan, with phases and decisions, is
`plans/ui-library-extraction.html` (published at https://faca9c667eb799c7.snist.dev).

## Product rules (never break these)

1. **No GPU device in the process, on any OS.** Draw on the CPU (today: software Direct2D on a WIC
   bitmap). Vendor drivers (`nv*umx`, `nvgpucomp64`, `amdxc64`, `atidx*`; Metal devices; libEGL,
   libGL, libvulkan) must never load. `tests/no_gpu_driver.rs` checks it on Windows.
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
- `mypaper` uses this crate as `mypaper_ui` (a renamed dependency) at about 1,000 call sites. Don't
  break public paths without a compatibility re-export, unless your brief says otherwise.

## Commands

```sh
cargo check -p stillkit --all-targets --features golden
cargo test -p stillkit --features golden
cargo clippy -p stillkit --all-targets --features golden
cargo run --release -p stillkit --example gallery      # docs/media/native/kit-*.png
STILLKIT_BLESS=1 cargo test -p stillkit --features golden   # rewrite goldens, then look at them
```

Use your own `CARGO_TARGET_DIR` under `C:\mypaper-targets\stillkit\` when you work beside others.
Don't run the mypaper app or deploy it. mypaper builds this checkout through a local `[patch]`
(its `.cargo/config.toml`), so a broken commit here breaks mypaper's next build.

## Git

- Commit with clear messages (`feat(el): …`, `refactor(platform): …`). Commit only your own paths.
- Never rewrite `main`'s history, and never force-push.
