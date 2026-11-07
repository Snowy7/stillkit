//! Screens in stillkit's coordinates: device pixels from the top-left of the primary screen.
//! AppKit's global space is points from the primary screen's bottom-left; positions convert at
//! the primary screen's backing scale (window contents use their own screen's scale), so a
//! second screen of another scale places windows by the primary's factor (docs/ui.md, "macOS
//! backend", to test). Off the main thread, where AppKit can't be asked, a 1920x1080 screen
//! at scale 1 stands in.
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSPoint, NSRect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    /// The work area (without the menu bar and Dock): left, top, right, bottom.
    pub work: crate::menu::Edges,
    pub dpi: u32,
}

/// The primary screen (the one with the menu bar at login): its size in device pixels, its
/// height in points and its scale.
#[derive(Debug, Clone, Copy)]
pub(super) struct Primary {
    pub size: (i32, i32),
    pub height: f64,
    pub scale: f64,
}

pub(super) fn primary() -> Primary {
    let fallback = Primary { size: (1920, 1080), height: 1080.0, scale: 1.0 };
    let Some(mtm) = MainThreadMarker::new() else { return fallback };
    let screens = NSScreen::screens(mtm);
    let Some(screen) = screens.firstObject() else { return fallback };
    let (frame, scale) = (screen.frame(), screen.backingScaleFactor());
    Primary {
        size: ((frame.size.width * scale).round() as i32, (frame.size.height * scale).round() as i32),
        height: frame.size.height,
        scale,
    }
}

/// A global AppKit point as device pixels from the primary screen's top-left.
pub(super) fn to_px(p: NSPoint) -> (i32, i32) {
    let s = primary();
    ((p.x * s.scale).round() as i32, ((s.height - p.y) * s.scale).round() as i32)
}

/// The AppKit frame (points, bottom-left origin) of a rectangle whose top-left is `at` device
/// pixels and whose size is `size` points.
pub(super) fn frame_at(at: (i32, i32), size: (f64, f64)) -> NSRect {
    let s = primary();
    NSRect::new(
        NSPoint::new(at.0 as f64 / s.scale, s.height - at.1 as f64 / s.scale - size.1),
        objc2_foundation::NSSize::new(size.0, size.1),
    )
}

/// An AppKit rectangle (global points) as device-pixel edges: left, top, right, bottom.
fn edges(r: NSRect) -> crate::menu::Edges {
    let (l, b) = to_px(r.origin);
    let (rt, t) = to_px(NSPoint::new(r.origin.x + r.size.width, r.origin.y + r.size.height));
    (l, t, rt, b)
}

/// The screen under a point (device pixels), or the primary one.
pub fn screen_at(x: i32, y: i32) -> Screen {
    let fallback = || {
        let p = primary();
        Screen { work: (0, 0, p.size.0, p.size.1), dpi: (96.0 * p.scale).round() as u32 }
    };
    let Some(mtm) = MainThreadMarker::new() else { return fallback() };
    let screens = NSScreen::screens(mtm);
    let under = screens.iter().find(|s| {
        let (l, t, r, b) = edges(s.frame());
        (l..r).contains(&x) && (t..b).contains(&y)
    });
    match under.or_else(|| screens.firstObject()) {
        Some(s) => Screen { work: edges(s.visibleFrame()), dpi: (96.0 * s.backingScaleFactor()).round() as u32 },
        None => fallback(),
    }
}

/// Whether the system lets things move (Reduce motion off).
pub(crate) fn animations_enabled() -> bool {
    !super::host::reduced_motion()
}
