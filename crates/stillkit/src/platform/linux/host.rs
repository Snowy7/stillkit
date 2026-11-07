//! Native host queries and an event-driven wait, with no idle timer.
/// Returns the output extent in physical pixels (global placement is compositor-owned).
pub fn screen_size() -> (i32, i32) {
    super::surface::screen_size()
}
/// Whether the compositor's XKB state currently has Shift active.
pub fn shift_down() -> bool {
    super::surface::shift_down()
}
/// Blocks on the Wayland display fd until an input or window event is pending.
pub fn wait() {
    super::surface::wait();
}
