//! Interaction feedback that moves without running the view: each control's hover and press
//! washes and its focus ring ease along the theme's interactive spring on the runtime's
//! clock. An entry exists only while a control is hot or still easing back to rest, so a
//! settled control costs nothing and no frame is asked for once everything rests. Reduced
//! motion lands every change at once.

use std::collections::HashMap;

use super::animate::Prop;
use crate::{HitId, Interaction, Rgba, Spring};

/// How far one control's feedback has come in, 0 (rest) to 1 (shown).
#[derive(Debug, Clone, Copy)]
struct Glow {
    hover: Prop<1>,
    press: Prop<1>,
    ring: Prop<1>,
}

impl Glow {
    fn rest() -> Self {
        Self {
            hover: Prop::at([0.0]),
            press: Prop::at([0.0]),
            ring: Prop::at([0.0]),
        }
    }

    fn moving(&self) -> bool {
        self.hover.moving.is_some() || self.press.moving.is_some() || self.ring.moving.is_some()
    }

    fn at_rest(&self) -> bool {
        !self.moving() && [self.hover.target, self.press.target, self.ring.target] == [[0.0]; 3]
    }
}

/// Every control's washes and focus ring, by hit.
#[derive(Debug, Default)]
pub(crate) struct Feedback {
    glows: HashMap<HitId, Glow>,
}

impl Feedback {
    /// Heads every control for what `ix` shows (hover and press washes) and `ring` (the hit
    /// whose focus ring shows), along `spring` (`None`: at once), from `start` seconds on the
    /// animation clock. Whether any target changed.
    pub fn sync(&mut self, ix: &Interaction, ring: Option<HitId>, spring: Option<Spring>, start: f64) -> bool {
        for id in [ix.hover, ix.pressed, ring].into_iter().flatten() {
            self.glows.entry(id).or_insert_with(Glow::rest);
        }
        let (wash, ring_spring) = (spring.map(Spring::calm), spring);
        let mut changed = false;
        for (&id, glow) in &mut self.glows {
            let hover = ix.hover == Some(id) && ix.pressed.is_none();
            let press = ix.hover == Some(id) && ix.pressed == Some(id);
            let shown = ring == Some(id);
            let before = (glow.hover.target, glow.press.target, glow.ring.target);
            glow.hover.set([f32::from(hover)], wash, start);
            glow.press.set([f32::from(press)], wash, start);
            glow.ring.set([f32::from(shown)], ring_spring, start);
            changed |= before != (glow.hover.target, glow.press.target, glow.ring.target);
        }
        self.glows.retain(|_, g| !g.at_rest());
        changed
    }

    /// Lands what has settled by `t` and forgets controls back at rest; whether anything
    /// still moves.
    pub fn settle(&mut self, t: f64) -> bool {
        let eps = 1.0 / 1024.0;
        let mut moving = false;
        for glow in self.glows.values_mut() {
            // Every value settles, so no short-circuiting.
            let m = [glow.hover.settle(t, eps), glow.press.settle(t, eps), glow.ring.settle(t, eps)];
            moving |= m.contains(&true);
        }
        self.glows.retain(|_, g| !g.at_rest());
        moving
    }

    pub fn moving(&self) -> bool {
        self.glows.values().any(Glow::moving)
    }

    /// Control `id`'s wash at `t`: `hover` and `press` (the theme's) blended by how far each
    /// has come in, in 8-bit steps (transparent at rest).
    pub fn wash(&self, id: HitId, t: f64, hover: Rgba, press: Rgba) -> Rgba {
        let Some(g) = self.glows.get(&id) else {
            return Rgba::TRANSPARENT;
        };
        let h = g.hover.value(t)[0].clamp(0.0, 1.0);
        let p = g.press.value(t)[0].clamp(0.0, 1.0);
        // At rest, the theme's own colours.
        match (h, p) {
            (_, 1.0) => return press,
            (1.0, 0.0) => return hover,
            _ => {}
        }
        let (ha, pa) = (hover.a * h * (1.0 - p), press.a * p);
        let alpha = quantize(ha + pa);
        if alpha <= 0.0 {
            return Rgba::TRANSPARENT;
        }
        let mix = |a: f32, b: f32| quantize((a * ha + b * pa) / (ha + pa));
        Rgba::new(mix(hover.r, press.r), mix(hover.g, press.g), mix(hover.b, press.b), alpha)
    }

    /// How far control `id`'s focus ring has come in at `t` (0 at rest, 1 shown; it may
    /// overshoot a little on the way).
    pub fn ring(&self, id: HitId, t: f64) -> f32 {
        self.glows.get(&id).map_or(0.0, |g| g.ring.value(t)[0].max(0.0))
    }
}

/// To the nearest 8-bit step, so a fade's slow tail repaints identical lists.
fn quantize(v: f32) -> f32 {
    (v * 255.0).round() / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hover_eases_in_and_out_and_then_forgets_the_control() {
        let mut fb = Feedback::default();
        let (hover, press) = (Rgba::new(1.0, 1.0, 1.0, 0.1), Rgba::new(1.0, 1.0, 1.0, 0.2));
        let ix = Interaction { hover: Some(7), ..Default::default() };
        assert!(fb.sync(&ix, None, Some(Spring::INTERACTIVE), 0.0));
        let early = fb.wash(7, 0.03, hover, press).a;
        assert!(early > 0.0 && early < 0.1, "{early}");
        assert!(fb.settle(0.03));
        assert!(!fb.settle(2.0));
        assert_eq!(fb.wash(7, 2.0, hover, press), hover);
        assert!(fb.sync(&Interaction::default(), None, Some(Spring::INTERACTIVE), 2.0));
        assert!(fb.moving());
        assert!(!fb.settle(4.0));
        assert!(fb.glows.is_empty(), "nothing is kept for a control at rest");
        // Without a spring (reduced motion) it lands at once.
        assert!(fb.sync(&ix, Some(7), None, 4.0));
        assert!(!fb.moving());
        assert_eq!((fb.wash(7, 4.0, hover, press), fb.ring(7, 4.0)), (hover, 1.0));
    }
}
