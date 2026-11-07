//! The images dyld has loaded, for the CPU-only check (`tests/macos_no_gpu_driver.rs`).
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_name(index: u32) -> *const c_char;
}

/// Every image (executable, dylibs, framework and driver bundles) loaded in this process.
pub fn loaded() -> Vec<String> {
    // SAFETY: dyld's image list; a name is null when the image unloaded meanwhile.
    let count = unsafe { _dyld_image_count() };
    (0..count)
        .filter_map(|i| {
            // SAFETY: an index below the count just read; the name is a C string dyld owns.
            let name = unsafe { _dyld_get_image_name(i) };
            // SAFETY: a non-null name is a NUL-terminated string dyld keeps while the image is loaded.
            (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
        })
        .collect()
}

/// A GPU driver: a Metal driver bundle (Apple silicon's AGX, Intel's and AMD's `MTLDriver`s,
/// loaded when a Metal device is made) or OpenGL's engine. Metal.framework itself is linked
/// by AppKit and QuartzCore and makes no device until asked.
pub fn is_gpu_driver(module: &str) -> bool {
    let m = module.to_ascii_lowercase();
    ["agxmetal", "mtldriver", "glengine"]
        .iter()
        .any(|s| m.contains(s))
}
