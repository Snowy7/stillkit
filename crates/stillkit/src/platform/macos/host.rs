//! The desktop queries an app host needs (`el::run`) and its idle wait: AppKit's
//! `nextEventMatchingMask` until a distant date (no wakes at all while idle), or until the one
//! deadline [`wake_at`] set, the run loop's own timeout.

use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSWorkspace};

use crate::el::Modifiers;

/// The primary screen's size in device pixels.
pub fn screen_size() -> (i32, i32) {
    let s = super::screen::primary();
    (s.size.0, s.size.1)
}

/// Whether Shift is held now.
pub fn shift_down() -> bool {
    modifiers().shift
}

/// The modifier keys held now. Command is `ctrl` (the shortcut key: Command-C copies, as
/// Ctrl+C does elsewhere), Option is `alt` and Control is `meta`.
pub fn modifiers() -> Modifiers {
    held(NSEvent::modifierFlags_class())
}

/// [`modifiers`] from an event's flags.
pub(super) fn held(f: NSEventModifierFlags) -> Modifiers {
    Modifiers {
        shift: f.contains(NSEventModifierFlags::Shift),
        ctrl: f.contains(NSEventModifierFlags::Command),
        alt: f.contains(NSEventModifierFlags::Option),
        meta: f.contains(NSEventModifierFlags::Control),
    }
}

/// Makes [`wait`] return by `deadline` though no event comes (`None`: only events wake it).
pub fn wake_at(deadline: Option<std::time::Instant>) {
    super::pump::set_wake(deadline);
}

/// Sleeps until an event arrives (or the [`wake_at`] deadline passes, or an animation's next
/// frame is due), then dispatches every event waiting.
pub fn wait() {
    super::pump::wait(None);
}

/// [`wait`], but for at most `timeout`.
pub fn wait_for(timeout: std::time::Duration) {
    super::pump::wait(Some(timeout));
}

/// Whether the user asked for less motion (System Settings > Accessibility > Display > Reduce
/// motion), read live.
pub fn reduced_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}
