use super::*;
/// A controlled numeric range. Rebuild from `on_change`'s clamped value.
pub struct Stepper<S> {
    label: Arc<str>,
    min: f64,
    max: f64,
    step: f64,
    value: f64,
    change: Option<Change<S, f64>>,
    disabled: bool,
}
impl<S: 'static> Stepper<S> {
    pub fn new(label: impl Into<Arc<str>>, min: f64, max: f64, step: f64, value: f64) -> Self {
        let min = if min.is_finite() { min } else { 0. };
        let max = if max.is_finite() { max.max(min) } else { min };
        Self {
            label: label.into(),
            min,
            max,
            step: if step.is_finite() && step > 0. {
                step
            } else {
                1.
            },
            value: if value.is_nan() {
                min
            } else {
                value.clamp(min, max)
            },
            change: None,
            disabled: false,
        }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_change(mut self, f: impl Fn(&mut S, f64) + 'static) -> Self {
        self.change = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let mut n = row()
            .gap(token::ITEM)
            .h(token::CONTROL)
            .radius(token::RADIUS_CONTROL)
            .fill(token::WELL)
            .align(Align::Center)
            .role(Role::SpinButton)
            .label(self.label.clone())
            .range(self.min, self.max, self.step, self.value)
            .disabled(self.disabled)
            .motion(token::MOTION_INTERACTIVE);
        if let Some(f) = self.change.clone() {
            let v = std::cell::Cell::new(self.value);
            let (min, max) = (self.min, self.max);
            n = n.on_step(move |s, d| {
                let next = (v.get() + d).clamp(min, max);
                v.set(next);
                f(s, next)
            });
        }
        for (delta, label) in [(-self.step, "Decrease"), (self.step, "Increase")] {
            if delta > 0. {
                n = n.child(
                    row()
                        .min_w(token::CONTROL)
                        .grow(1.)
                        .justify(Justify::Center)
                        .align(Align::Center)
                        .child(
                            text(self.value.to_string())
                                .font(TextRole::Body)
                                .color(LABEL)
                                .opacity(if self.disabled { 0.45 } else { 1. }),
                        ),
                );
            }
            let next = (self.value + delta).clamp(self.min, self.max);
            let mut b = IconButton::new(
                if delta < 0. { Icon::Remove } else { Icon::Add },
                format!("{label} {}", self.label),
            )
            .disabled(self.disabled || next == self.value);
            if let Some(f) = self.change.clone() {
                b = b.on_press(move |s| f(s, next));
            }
            n = n.child(
                b.build()
                    .key(label)
                    .label(format!("{label} {}", self.label)),
            );
        }
        n
    }
}
