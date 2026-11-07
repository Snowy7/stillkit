//! Process mapping audit for the CPU-only boundary.
/// Loaded.
pub fn loaded() -> Vec<String> {
    std::fs::read_to_string("/proc/self/maps")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            l.split_whitespace()
                .last()
                .filter(|p| p.starts_with('/'))
                .map(str::to_owned)
        })
        .collect()
}
/// Whether gpu driver.
pub fn is_gpu_driver(module: &str) -> bool {
    let m = module.to_ascii_lowercase();
    [
        "libegl",
        "libgl.",
        "libglx",
        "libgldispatch",
        "libvulkan",
        "libgbm",
        "radeonsi",
        "libgallium",
        "_dri.so",
        "libdrm_amdgpu",
        "nvidia",
    ]
    .iter()
    .any(|s| m.contains(s))
}
