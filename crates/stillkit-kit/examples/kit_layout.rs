//! Headless layout/data gallery; no native window or GPU.
mod layout_support;
fn main() -> anyhow::Result<()> {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/media/kit");
    for (name, theme) in layout_support::themes() {
        let path = out.join(format!("layout-{name}.png"));
        stillkit::golden::write(&layout_support::sheet(theme)?, &path)?;
        println!("{}", path.display());
    }
    Ok(())
}
