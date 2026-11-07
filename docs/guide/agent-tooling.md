# Agent tooling

el::run provides --snap image.png, --tree tree.json and --script input.json without creating a native window. CPU snapshots need no GPU. --scale changes raster density; --at selects seconds on the virtual clock; --reduced checks reduced motion.

Scripts are arrays of timed actions: click by key, move/down/up coordinates, right click, key, type or wheel. Coordinates are design pixels. Tree dumps include bounds, semantics and current values; secure fields withhold text. Headless scripts use a memory clipboard. Stable keys and accessible labels let agents address controls. This example uses the same Runtime::play path as the CLI.

```rust
use std::time::Duration;
use stillkit::el::{Runtime, Script, col, text};
use stillkit::{Scheme, Theme};
fn main() -> anyhow::Result<()> {
    let mut rt = Runtime::new(
        0u32,
        |count| {
            col().pad(20.).child(
                text(format!("Count: {count}"))
                    .key("count")
                    .on_press(|count: &mut u32| *count += 1),
            )
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (320, 180),
        1.,
    )?;
    rt.frame(Duration::ZERO);
    rt.play(&Script::parse(r#"[{"at":0.1,"click":"count"}]"#)?, None);
    assert!(rt.find("count").is_some());
    assert_eq!(*rt.state(), 1);
    Ok(())
}
```

[Executable source](../../crates/stillkit/examples/guide_agent_tooling.rs).

```sh
cargo run -p stillkit-kit --example showcase -- --snap app.png --tree app.json
cargo run -p stillkit-kit --example showcase -- --script input.json --at 0.5 --snap after.png
```

input.json:

```json
[{"at":0.1,"click":"name"}]
```

[All guides](../../README.md#guides) ? [Element reference](../el.md)
