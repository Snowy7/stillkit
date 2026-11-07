//! Compile-only app host queries (see the Windows backend's `host`).

pub fn screen_size() -> (i32, i32) {
    unimplemented!("screen metrics")
}
pub fn shift_down() -> bool {
    unimplemented!("keyboard state")
}
pub fn modifiers() -> crate::el::Modifiers {
    unimplemented!("keyboard state")
}
pub fn wake_at(_: Option<std::time::Instant>) {
    unimplemented!("native event wait")
}
pub fn wait() {
    unimplemented!("native event wait")
}
/// No system setting to read: full motion.
pub fn reduced_motion() -> bool {
    false
}
