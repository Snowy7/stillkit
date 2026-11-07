//! A scroll view's retained state ([`super::El::scroll`]): where its content rests, the spring
//! the offset glides along, and its overlay scrollbar's fade. Layout never sees the offset:
//! paint moves the view's children back by it, so scrolling neither runs the view nor
//! re-lays anything out, and only the view's clipped group changes.
//!
//! Times are the animator's clock (seconds, time-scaled); offsets are device px along the
//! view's axis, shown on whole pixels.

use super::animate::Prop;
use super::element::Axis;
use crate::{Point, Rect, Spring, Theme};

/// Design px one wheel notch scrolls (the old kit's [`crate::scroll::NOTCH`]).
pub(crate) const NOTCH: f32 = crate::scroll::NOTCH;
/// Design px an arrow key scrolls.
pub(crate) const LINE: f32 = 40.0;

#[derive(Debug, Clone)]
pub(crate) struct ScrollView {
    pub axis: Axis,
    /// The offset; its target is where the content rests.
    offset: Prop<1>,
    /// How far it can scroll: content less viewport.
    pub max: f32,
    /// The viewport's length along the axis.
    pub viewport: f32,
    /// When it last scrolled, for the scrollbar.
    active: Option<f64>,
    /// The offset and scrollbar opacity in the last paint.
    pub shown: (f32, f32),
}

impl ScrollView {
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            offset: Prop::at([0.0]),
            max: 0.0,
            viewport: 0.0,
            active: None,
            shown: (0.0, 0.0),
        }
    }

    /// A new layout: the view is `rect` with content `extent` (device px). A shorter content
    /// pulls the offset back at once.
    pub fn laid(&mut self, rect: Rect, extent: (f32, f32), t: f64) {
        let (viewport, content) = match self.axis {
            Axis::Vertical => (rect.height(), extent.1),
            Axis::Horizontal => (rect.width(), extent.0),
        };
        self.viewport = viewport;
        self.max = (content - viewport).max(0.0).round();
        if self.offset.target[0] > self.max {
            self.offset.set([self.max], None, t);
        }
    }

    /// Where the content rests.
    pub fn target(&self) -> f32 {
        self.offset.target[0]
    }

    /// The offset to draw at `t`, whole pixels.
    pub fn offset(&self, t: f64) -> f32 {
        self.offset.value(t)[0].clamp(0.0, self.max).round()
    }

    /// The offset as a point (how far its children move back).
    pub fn shift(&self, t: f64) -> Point {
        let by = self.offset(t);
        match self.axis {
            Axis::Vertical => Point::new(0.0, by),
            Axis::Horizontal => Point::new(by, 0.0),
        }
    }

    /// Heads for `to` (clamped), along `spring` or at once. Whether its rest moved.
    pub fn to(&mut self, to: f32, spring: Option<Spring>, t: f64) -> bool {
        let to = to.clamp(0.0, self.max).round();
        if to == self.target() {
            return false;
        }
        self.offset.set([to], spring, t);
        self.active = Some(t);
        true
    }

    /// Moves by `by` device px.
    pub fn by(&mut self, by: f32, spring: Option<Spring>, t: f64) -> bool {
        self.to(self.target() + by, spring, t)
    }

    /// Scrolls just enough to show `start..end` (content px along the axis, from the
    /// content's start); whether it moved.
    pub fn reveal(&mut self, start: f32, end: f32, spring: Option<Spring>, t: f64) -> bool {
        let at = self.target();
        if start < at {
            self.to(start, spring, t)
        } else if end > at + self.viewport {
            self.to((end - self.viewport).min(start), spring, t)
        } else {
            false
        }
    }

    /// The scrollbar's opacity at `t`, on 8-bit steps: shown while it scrolls, then fading.
    pub fn bar(&self, t: f64, theme: &Theme) -> f32 {
        let Some(active) = self.active.filter(|_| self.max > 0.0) else {
            return 0.0;
        };
        let (idle, fade) = (f64::from(theme.motion.scrollbar_idle), f64::from(theme.motion.scrollbar_fade));
        let since = t - active - idle;
        if self.offset.moving.is_some() || since <= 0.0 {
            return 1.0;
        }
        let alpha = (1.0 - since / fade.max(0.001)).clamp(0.0, 1.0) as f32;
        (alpha * 255.0).round() / 255.0
    }

    /// Lands the glide once it settles and forgets the scrollbar once it faded; whether
    /// anything still moves (or waits to fade).
    pub fn settle(&mut self, t: f64, theme: &Theme) -> bool {
        let gliding = self.offset.settle(t, 0.25);
        if !gliding && self.active.is_some() && self.bar(t, theme) == 0.0 && self.shown.1 == 0.0 {
            self.active = None;
        }
        gliding || self.active.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::new(crate::Style::Liquid, crate::Scheme::Dark, crate::theme::SYSTEM_BLUE)
    }

    #[test]
    fn it_glides_within_its_content_and_its_bar_fades_then_rests() {
        let theme = theme();
        let mut v = ScrollView::new(Axis::Vertical);
        v.laid(Rect::new(0.0, 0.0, 100.0, 200.0), (100.0, 1000.0), 0.0);
        assert_eq!(v.max, 800.0);
        assert!(!v.settle(0.0, &theme), "nothing to do at rest");
        assert!(v.by(48.0, Some(theme.motion.scroll), 0.0));
        assert_eq!(v.offset(0.0), 0.0, "a glide starts where it was");
        assert_eq!(v.offset(1.0), 48.0);
        assert!(v.by(-100.0, None, 1.0));
        assert_eq!(v.target(), 0.0, "clamped at the top");
        assert!(!v.by(-1.0, None, 1.0), "nothing above the top");
        assert!(v.to(5000.0, None, 1.0));
        assert_eq!(v.target(), 800.0, "clamped at the end");
        assert_eq!(v.bar(1.0, &theme), 1.0);
        let gone = 1.0 + f64::from(theme.motion.scrollbar_idle + theme.motion.scrollbar_fade) + 0.01;
        assert_eq!(v.bar(gone, &theme), 0.0);
        v.shown.1 = 0.0;
        assert!(!v.settle(gone, &theme), "rests once the bar faded");
        v.laid(Rect::new(0.0, 0.0, 100.0, 200.0), (100.0, 300.0), gone);
        assert_eq!(v.target(), 100.0, "shorter content pulls it back");
    }
}
