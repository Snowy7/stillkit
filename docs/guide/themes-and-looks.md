# Themes and looks

Liquid, One UI, Solid, Fluent and Mono provide light/dark presets. Each Theme bundles independent material, shape, type ramp, spacing, palette, motion and icons. Builder calls mix parts and recompute derived values. check() returns diagnostics: inspect them before shipping a custom mix.

Enable stillkit's serde feature for Theme::from_toml/from_json. Documents choose a base preset and override parts; unknown keys are rejected. Read file contents yourself and pass the string to from_toml (there is no from_file API). Run this example with --features serde to exercise loading. See [theme schema](../ui.md#looks).

```rust
use stillkit::theme::{Material, Shape};
use stillkit::{Scheme, Theme};
fn main() -> anyhow::Result<()> {
    let theme = Theme::one_ui(Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
        .material(Material::liquid())
        .shape(Shape::one_ui());
    println!("Mix diagnostics: {:?}", theme.check());
    #[cfg(feature = "serde")]
    {
        let loaded = Theme::from_toml(
            r#"
base = "oneui"
scheme = "dark"
[material]
base = "liquid"
"#,
        )?;
        println!("Loaded diagnostics: {:?}", loaded.check());
    }
    Ok(())
}
```

[Executable source](../../crates/stillkit/examples/guide_themes_and_looks.rs).


[All guides](../../README.md#guides) ? [Element reference](../el.md)
