//! Render every implemented component through Runtime::snap, without a window or GPU.
mod support;
fn main() -> anyhow::Result<()> {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/media/kit");
    for (name, theme) in support::themes() {
        let path = out.join(format!("kit-{name}.png"));
        stillkit::golden::write(&support::sheet(theme)?, &path)?;
        println!("{}", path.display());
    }
    Ok(())
}
