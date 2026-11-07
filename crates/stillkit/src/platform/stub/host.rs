//! Compile-only app host queries (see the Windows backend's `host`).

pub fn screen_size() -> (i32, i32) {
    unimplemented!("screen metrics")
}
pub fn shift_down() -> bool {
    unimplemented!("keyboard state")
}
pub fn wait() {
    unimplemented!("native event wait")
}
