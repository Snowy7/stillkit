# CI budget and verification

Pushes to main and pull requests run three jobs. Concurrency cancels obsolete runs for the same ref. Each job uses the toolchain and components from rust-toolchain.toml and caches Cargo dependencies with Swatinem/rust-cache. The nightly channel currently floats, so a nightly update can invalidate caches.

- Windows: CPU golden/serde workspace tests, then all-target Clippy including GPU code with warnings denied. Build output and cache live on D:. LLVM avoids the observed Cranelift Windows exception failure mode.
- Ubuntu: Wayland, xkbcommon, fontconfig and fonts installed; CPU golden/serde workspace tests run headless without a compositor.
- macOS check: Ubuntu cross-checks aarch64-apple-darwin, no linker or native runner needed. Advisory (`continue-on-error`) until the backend lands.

GPU integration tests `gpu_driver.rs` and `gpu_render.rs` require stillkit's gpu feature. The test command omits it, so these tests are cleanly excluded. Clippy compiles them but does not execute them. stillkit-glass workspace unit tests exercise CPU-side scene/material/motion logic without creating a device. GPU pixel parity and real driver behavior remain checks for a GPU-equipped machine.

Planning estimate per push with warm caches: Windows 2-5 runner minutes, Linux 3-5, macOS cross-check 2-3. With Windows counted twice, budget roughly **9-18 billable minutes**; cold caches may take **20-40**. These are estimates, not hosted measurements: adjust after the first Actions runs. Three independent jobs keep elapsed time near the longest job; cancellation avoids paying for superseded commits. No Actions run was started by this change.

Local verification on Windows, rustc 1.101.0-nightly (1c3a65069, compiler commit date 2026-11-02), CARGO_TARGET_DIR=D:\stillkit-targets\O, CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm:

```text
cargo test --workspace --features stillkit/golden,stillkit/serde
  256 passed, 0 failed, 1 existing ignored doctest
cargo clippy --workspace --all-targets --features stillkit/golden,stillkit/serde,stillkit/gpu -- -D warnings
  passed
cargo check --workspace --target x86_64-unknown-linux-gnu
  passed, existing accessibility dead-code warnings
cargo check --workspace --target aarch64-apple-darwin
  passed, existing stub/dead-code warnings
actionlint .github/workflows/ci.yml
  passed
```

All eleven guide examples and showcase compiled. Nine windowed guide examples also produced headless PNG/tree output; the theme example loaded its TOML with serde and returned no check() diagnostics, and the tooling example replayed a click and asserted updated state. README's hello matches guide_getting_started.

Showcase PNGs were visually inspected in Solid and One UI + Liquid material, each light and dark. Additional script captures exercised devices, dialog, toast, select, editing and live theme changes; tree assertions checked their results. Local artifacts are under D:\stillkit-targets\O (showcase-review.png is the contact sheet). These are CPU captures of themed materials; actual glass lenses require Surface hooks, which el::run does not attach automatically. Linux/macOS native execution was not tested on this Windows machine.
