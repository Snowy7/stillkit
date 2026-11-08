//! Compile-only app host queries (see the Windows backend's `host`).

/// Screen size.
pub fn screen_size() -> (i32, i32) {
    unimplemented!("screen metrics")
}
/// Shift down.
pub fn shift_down() -> bool {
    unimplemented!("keyboard state")
}
/// Modifiers.
pub fn modifiers() -> crate::el::Modifiers {
    unimplemented!("keyboard state")
}
/// Wake at.
pub fn wake_at(_: Option<std::time::Instant>) {
    unimplemented!("native event wait")
}
/// Wait.
pub fn wait() {
    unimplemented!("native event wait")
}
/// No system setting to read: full motion.
pub fn reduced_motion() -> bool {
    false
}
/// Compile-only waker half.
pub(crate) struct Poster;
impl Poster {
    pub(crate) fn new() -> Self {
        Self
    }
    pub(crate) fn post(&self) {}
}
