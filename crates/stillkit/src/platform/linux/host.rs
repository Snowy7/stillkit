//! Native host queries and an event-driven wait, with no idle timer.
/// Returns the output extent in physical pixels (global placement is compositor-owned).
pub fn screen_size() -> (i32, i32) {
    super::surface::screen_size()
}
/// Whether the compositor's XKB state currently has Shift active.
pub fn shift_down() -> bool {
    super::surface::shift_down()
}
/// The modifier keys the compositor's XKB state has active.
pub fn modifiers() -> crate::el::Modifiers {
    super::surface::modifiers()
}
/// Makes [`wait`] return by `deadline` though no event comes (`None`: only events wake it):
/// a timerfd beside the display fd, disarmed when nothing is pending.
pub fn wake_at(deadline: Option<std::time::Instant>) {
    super::wake::set(deadline);
}
/// Blocks on the Wayland display fd until an input or window event is pending, or the
/// [`wake_at`] deadline passes.
pub fn wait() {
    super::surface::wait();
}

/// Whether the user asked for less motion: GNOME's `enable-animations` set to false (Settings >
/// Accessibility > Reduce animation). Read once through `gsettings`; elsewhere, or when it
/// can't be read, motion stays on.
pub fn reduced_motion() -> bool {
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(|| {
        std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
            .is_ok_and(|out| out.status.success() && out.stdout.trim_ascii() == b"false")
    })
}
