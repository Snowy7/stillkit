# Getting started

Install Rust with rustup. Clone this repository to run the examples. For a downstream app, create a Rust binary:

```sh
cargo new hello-stillkit
cd hello-stillkit
```

Keep edition = "2024" in Cargo.toml and add these dependencies, pinning YOUR_COMMIT to a tested revision:

```toml
[dependencies]
anyhow = "1"
stillkit = { git = "https://github.com/Snowy7/stillkit", rev = "YOUR_COMMIT" }
# Optional ready-made components, from the same revision:
stillkit-kit = { git = "https://github.com/Snowy7/stillkit", rev = "YOUR_COMMIT" }

[patch.crates-io]
accesskit_windows = { git = "https://github.com/Snowy7/stillkit", rev = "YOUR_COMMIT" }
```

Cargo applies only root-workspace patches, so downstream Windows apps need that AccessKit patch too.

The repo's rust-toolchain.toml selects nightly with Cranelift, rust-src, clippy, rustfmt and rust-analyzer. It is currently floating, not date-pinned. Verified compiler: rustc 1.101.0-nightly (commit date 2026-11-02). `.cargo/config.toml` uses Cranelift for local dev crates, LLVM for dependencies and nightly parallel frontend flags. Downstream git dependencies inherit neither this config nor this toolchain override: the crates build on stable Rust 1.96 or newer with ordinary LLVM defaults. In this checkout, set CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm to opt out of Cranelift; the nightly rustflags still apply.

Linux (Debian/Ubuntu): install pkg-config, libwayland-dev, libxkbcommon-dev and libfontconfig1-dev. Interactive CPU windows need a Wayland session; snapshots do not. Put the following in src/main.rs:

```rust
use stillkit::el::{self, App, El, col, text, token};
use stillkit::{Kind, Scheme, Theme};
fn view(count: &u32) -> El<u32> {
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(text(format!("Hello, stillkit! {count}")))
        .child(
            text("Click to count")
                .key("count")
                .on_press(|count: &mut u32| *count += 1),
        )
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Hello",
        size: (360.0, 240.0),
        kind: Kind::App { min: (240, 160) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: 0,
        view,
    })
}
```

Run your downstream app with `cargo run`, or `cargo run -- --snap app.png --tree app.json` for headless output.

From the stillkit checkout, run the [executable source](../../crates/stillkit/examples/guide_getting_started.rs) instead:

```sh
cargo run -p stillkit --example guide_getting_started -- --snap app.png --tree app.json
cargo run -p stillkit --example guide_getting_started -- --script input.json --at 0.5 --snap after.png
```

input.json:

```json
[{"at":0.1,"click":"count"}]
```

[All guides](../../README.md#guides) | [Element reference](../el.md)
