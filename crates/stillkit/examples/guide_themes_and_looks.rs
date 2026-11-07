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
