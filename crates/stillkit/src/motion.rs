//! Motion: springs (ported from `bars/shell/src/style/motion.ts`), values moving along them, and
//! the frame ticker, a high-resolution waitable timer armed only while something moves.
//!
//! Slides and fades change a surface's position and alpha only ([`crate::Surface::move_to`],
//! [`crate::Surface::set_alpha`]); nothing is redrawn for them.

use std::time::{Duration, Instant};

/// A damped spring (m·x'' + c·x' + k·(x − 1) = 0), released from 0 at rest toward 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub mass: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    /// motion.ts `spring`.
    pub const DEFAULT: Self = Self::new(1.0, 220.0, 22.0);
    /// motion.ts `spring-soft`.
    pub const SOFT: Self = Self::new(1.0, 120.0, 17.0);
    /// motion.ts `spring-bouncy`.
    pub const BOUNCY: Self = Self::new(1.0, 260.0, 13.0);

    pub const fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            mass,
            stiffness,
            damping,
        }
    }

    /// A spring in Apple's terms (SwiftUI `spring(duration:bounce:)`), as liquid-rust models
    /// it (github.com/al1h3n/liquid-rust, `src/motion.rs`, MIT/Apache-2.0): `duration` is
    /// the perceptual settle time in seconds, `bounce` 0 for none, up to 0.95 for a lot,
    /// negative for an over-damped curve. ω₀ = 2π / duration; ζ = 1 − bounce (bounce ≥ 0)
    /// or 1 / (1 + bounce).
    pub const fn apple(duration: f32, bounce: f32) -> Self {
        let bounce = if bounce > 0.95 {
            0.95
        } else if bounce < -0.95 {
            -0.95
        } else {
            bounce
        };
        let duration = if duration < 0.001 { 0.001 } else { duration };
        let w0 = std::f32::consts::TAU / duration;
        let zeta = if bounce >= 0.0 {
            1.0 - bounce
        } else {
            1.0 / (1.0 + bounce)
        };
        Self::new(1.0, w0 * w0, 2.0 * zeta * w0)
    }

    /// SwiftUI `.smooth`: no overshoot.
    pub const SMOOTH: Self = Self::apple(0.5, 0.0);
    /// SwiftUI `.snappy`: a little overshoot, for layout and morphs.
    pub const SNAPPY: Self = Self::apple(0.5, 0.15);
    /// SwiftUI `.interactiveSpring`: tight follow of a press or a drag.
    pub const INTERACTIVE: Self = Self::apple(0.15, 0.14);

    /// The position `t` seconds after release.
    pub fn at(&self, t: f32) -> f32 {
        let (m, k, c, t) = (
            f64::from(self.mass),
            f64::from(self.stiffness),
            f64::from(self.damping),
            f64::from(t),
        );
        let w0 = (k / m).sqrt();
        let zeta = c / (2.0 * (k * m).sqrt());
        let x = if zeta < 1.0 {
            let wd = w0 * (1.0 - zeta * zeta).sqrt();
            1.0 - (-zeta * w0 * t).exp() * ((wd * t).cos() + (zeta * w0 / wd) * (wd * t).sin())
        } else if zeta == 1.0 {
            1.0 - (-w0 * t).exp() * (1.0 + w0 * t)
        } else {
            let root = (zeta * zeta - 1.0).sqrt();
            let r1 = -w0 * (zeta - root);
            let r2 = -w0 * (zeta + root);
            1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
        };
        x as f32
    }

    /// The displacement from rest and the velocity `t` seconds after letting go at displacement
    /// `d0` moving at `v0` (units per second). Exact in every damping regime, so a value
    /// retargeted mid-flight restarts from its true position and velocity:
    /// [`at`](Self::at)`(t) == 1 - released(1.0, 0.0, t).0`.
    pub fn released(&self, d0: f32, v0: f32, t: f32) -> (f32, f32) {
        let (m, k, c) = (
            f64::from(self.mass),
            f64::from(self.stiffness),
            f64::from(self.damping),
        );
        let (d0, v0, t) = (f64::from(d0), f64::from(v0), f64::from(t));
        let w0 = (k / m).sqrt();
        let zeta = c / (2.0 * (k * m).sqrt());
        let (d, v) = if (zeta - 1.0).abs() < 1e-6 {
            let b = v0 + w0 * d0;
            let e = (-w0 * t).exp();
            (e * (d0 + b * t), e * (b - w0 * (d0 + b * t)))
        } else if zeta < 1.0 {
            let a = zeta * w0;
            let wd = w0 * (1.0 - zeta * zeta).sqrt();
            let b = (v0 + a * d0) / wd;
            let (e, (sin, cos)) = ((-a * t).exp(), (wd * t).sin_cos());
            (
                e * (d0 * cos + b * sin),
                e * ((wd * b - a * d0) * cos - (a * b + wd * d0) * sin),
            )
        } else {
            let root = (zeta * zeta - 1.0).sqrt();
            let (r1, r2) = (-w0 * (zeta - root), -w0 * (zeta + root));
            let a = (v0 - r2 * d0) / (r1 - r2);
            let b = d0 - a;
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            (a * e1 + b * e2, a * r1 * e1 + b * r2 * e2)
        };
        (d as f32, v as f32)
    }

    /// The same spring with no overshoot: damping raised to critical when it is below.
    /// Colours and opacity move along this, so they never pass their target.
    pub fn calm(self) -> Self {
        let critical = 2.0 * (self.stiffness * self.mass).sqrt();
        Self {
            damping: self.damping.max(critical),
            ..self
        }
    }

    /// The same curve `by` times as fast (the same bounce, `1 / by` of the settle time).
    pub fn faster(self, by: f32) -> Self {
        Self {
            stiffness: self.stiffness * by * by,
            damping: self.damping * by,
            ..self
        }
    }

    /// Seconds until the spring stays within `epsilon` of rest (sampled every millisecond,
    /// at most 3 s).
    pub fn settle(&self, epsilon: f32) -> f32 {
        let last = (0..=3000)
            .rev()
            .map(|ms| ms as f32 / 1000.0)
            .find(|&t| (self.at(t) - 1.0).abs() > epsilon)
            .unwrap_or(0.0);
        (last + 0.001).min(3.0)
    }
}

/// A value moving from `from` to `to` along a spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    spring: Spring,
    start: Instant,
    duration: Duration,
}

impl Tween {
    /// Starts at `now`. It ends when the spring settles within 1/1000 of the distance.
    pub fn new(from: f32, to: f32, spring: Spring, now: Instant) -> Self {
        Self {
            from,
            to,
            spring,
            start: now,
            duration: Duration::from_secs_f32(spring.settle(0.001)),
        }
    }

    pub fn value(&self, now: Instant) -> f32 {
        if self.done(now) {
            return self.to;
        }
        let t = now.saturating_duration_since(self.start).as_secs_f32();
        self.from + (self.to - self.from) * self.spring.at(t)
    }

    pub fn done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= self.duration
    }
}

pub use crate::platform::current::timing::{Ticker, display_frame};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn springs_start_at_rest_and_settle_on_one_in_every_damping_regime() {
        for spring in [
            Spring::DEFAULT,
            Spring::BOUNCY,
            Spring::new(1.0, 100.0, 20.0), // critically damped
            Spring::new(1.0, 100.0, 40.0), // overdamped
        ] {
            assert!(spring.at(0.0).abs() < 1e-6, "{spring:?}");
            let settle = spring.settle(0.001);
            assert!(settle > 0.05 && settle < 3.0, "{spring:?}: {settle}");
            assert!((spring.at(settle) - 1.0).abs() <= 0.001, "{spring:?}");
        }
        // Underdamped springs overshoot; the critically damped one never does.
        assert!((0..1000).any(|ms| Spring::BOUNCY.at(ms as f32 / 1000.0) > 1.05));
        let critical = Spring::new(1.0, 100.0, 20.0);
        assert!((0..1000).all(|ms| critical.at(ms as f32 / 1000.0) <= 1.0));
        // motion.ts: the default spring settles in about two thirds of a second.
        let settle = Spring::DEFAULT.settle(0.001);
        assert!((0.5..0.8).contains(&settle), "{settle}");
    }

    #[test]
    fn a_tween_ends_exactly_on_its_target() {
        let start = Instant::now();
        let tween = Tween::new(-36.0, 0.0, Spring::DEFAULT, start);
        assert_eq!(tween.value(start), -36.0);
        let later = start + Duration::from_secs(1);
        assert!(tween.done(later));
        assert_eq!(tween.value(later), 0.0);
    }

    #[test]
    fn released_matches_the_step_response_and_its_own_velocity() {
        for spring in [
            Spring::DEFAULT,
            Spring::BOUNCY,
            Spring::new(1.0, 100.0, 20.0),
            Spring::new(1.0, 100.0, 40.0),
        ] {
            let (d, v) = spring.released(3.0, -40.0, 0.0);
            assert!((d - 3.0).abs() < 1e-5 && (v + 40.0).abs() < 1e-3, "{spring:?}");
            for ms in [0u16, 30, 120, 400] {
                let t = f32::from(ms) / 1000.0;
                let (d, _) = spring.released(1.0, 0.0, t);
                assert!((1.0 - d - spring.at(t)).abs() < 1e-5, "{spring:?} at {t}");
                // The velocity is the slope of the position.
                let h = 1e-3;
                let (a, _) = spring.released(3.0, -40.0, t);
                let (b, _) = spring.released(3.0, -40.0, t + h);
                let (_, v) = spring.released(3.0, -40.0, t + h / 2.0);
                assert!(((b - a) / h - v).abs() < 0.05 * v.abs().max(1.0), "{spring:?} at {t}");
            }
        }
        let calm = Spring::BOUNCY.calm();
        assert!((0..1000).all(|ms| calm.at(ms as f32 / 1000.0) <= 1.0 + 1e-6));
        let fast = Spring::DEFAULT.faster(2.0);
        assert!((fast.settle(0.001) * 2.0 - Spring::DEFAULT.settle(0.001)).abs() < 0.01);
    }

}
