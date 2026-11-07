//! Compares two directories of gallery PNGs (CPU and GPU renders, docs/ui.md "Render modes"):
//! per page, whether the files are byte-identical, the largest channel difference, and the share
//! of channels that differ by more than 4 levels.
//!
//! ```text
//! cargo run --release -p stillkit --example glass_compare -- <dir a> <dir b>
//! ```

use std::path::Path;

fn decode(path: &Path) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut pixels)?;
    pixels.truncate(info.buffer_size());
    Ok((info.width, info.height, pixels))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() == 2, "usage: glass_compare <dir a> <dir b>");
    let (a, b) = (Path::new(&args[0]), Path::new(&args[1]));
    let mut names: Vec<_> = std::fs::read_dir(a)?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with("kit-") && n.ends_with(".png"))
        .collect();
    names.sort();
    println!("| Page | Same bytes | Max difference | Channels over 4 |");
    println!("| --- | --- | --- | --- |");
    for name in names {
        let (pa, pb) = (a.join(&name), b.join(&name));
        if !pb.exists() {
            println!("| {name} | missing | | |");
            continue;
        }
        let same = std::fs::read(&pa)? == std::fs::read(&pb)?;
        let ((wa, ha, x), (wb, hb, y)) = (decode(&pa)?, decode(&pb)?);
        anyhow::ensure!((wa, ha) == (wb, hb), "{name}: sizes differ");
        let max = x.iter().zip(&y).map(|(p, q)| p.abs_diff(*q)).max().unwrap_or(0);
        let over = x.iter().zip(&y).filter(|(p, q)| p.abs_diff(**q) > 4).count() as f64 / x.len() as f64;
        println!("| {name} | {} | {max} | {:.3} % |", if same { "yes" } else { "no" }, over * 100.0);
    }
    Ok(())
}
