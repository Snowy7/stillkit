use super::*;
use std::time::Duration;
use stillkit::el::{arc, image};

/// A determinate meter with a spring-driven arc and centred percentage.
pub struct Ring {
    label: Arc<str>,
    value: f32,
    size: f32,
}
impl Ring {
    pub fn new(label: impl Into<Arc<str>>, value: f32) -> Self {
        Self {
            label: label.into(),
            value: unit(value),
            size: 72.,
        }
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(24.);
        self
    }
    pub fn build<S>(self) -> El<S> {
        arc(self.value)
            .size(self.size)
            .stroke(token::RING)
            .track(token::FILL)
            .motion(token::MOTION_INTERACTIVE)
            .role(Role::Meter)
            .label(self.label)
            .range(0., 1., 0.01, self.value.into())
            .value(format!("{}%", (self.value * 100.).round()))
            .align(Align::Center)
            .justify(Justify::Center)
            .child(
                text(format!("{}%", (self.value * 100.).round()))
                    .color(LABEL)
                    .font(TextRole::Headline),
            )
    }
}
/// Controlled indeterminate activity. Increment `phase` in `on_tick`, and rebuild.
/// Set `visible(false)` whenever hidden, including by host navigation/viewport state.
/// A removed/hidden spinner owns no timeout; reduced motion shows a stationary arc.
pub struct Spinner<S> {
    label: Arc<str>,
    phase: u64,
    visible: bool,
    tick: Option<Press<S>>,
    reduced: bool,
}
impl<S: 'static> Spinner<S> {
    pub fn new(label: impl Into<Arc<str>>, phase: u64) -> Self {
        Self {
            label: label.into(),
            phase,
            visible: true,
            tick: None,
            reduced: stillkit::el::reduced_motion(),
        }
    }
    /// Match a host/runtime reduced-motion override as well as the system default.
    pub fn reduced_motion(mut self, reduced: bool) -> Self {
        self.reduced = reduced;
        self
    }
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
    pub fn on_tick(mut self, tick: impl Fn(&mut S) + 'static) -> Self {
        self.tick = Some(Box::new(tick));
        self
    }
    pub fn build(self) -> El<S> {
        if !self.visible {
            return el();
        }
        let reduced = self.reduced;
        let phase = if reduced { 0 } else { self.phase % 4 };
        let mut n = arc(0.27)
            .motion(token::MOTION_INTERACTIVE)
            .no_track()
            .size(24.)
            .stroke(token::RING)
            .arc_start(match phase {
                1 => stillkit::el::ArcStart::Right,
                2 => stillkit::el::ArcStart::Bottom,
                3 => stillkit::el::ArcStart::Left,
                _ => stillkit::el::ArcStart::Top,
            })
            .role(Role::ProgressIndicator)
            .label(self.label)
            .value("Loading");
        if !reduced && let Some(tick) = self.tick {
            n = n.child(
                el().key(format!("tick-{}", self.phase))
                    .on_timeout(Duration::from_millis(120), tick),
            );
        }
        n
    }
}
/// Quiet, static placeholders: no idle shimmer or timer.
pub struct Skeleton {
    width: f32,
    height: f32,
}
impl Skeleton {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
    pub fn build<S>(self) -> El<S> {
        el().w(self.width)
            .h(self.height)
            .radius(token::RADIUS_CONTROL)
            .fill(token::FILL)
            .role(Role::Label)
            .label("Loading placeholder")
    }
}
/// Circular monogram or bitmap with an optional labelled status dot.
pub struct Avatar {
    label: Arc<str>,
    monogram: Arc<str>,
    image: Option<Arc<stillkit::Image>>,
    size: f32,
    status: Option<Arc<str>>,
}
impl Avatar {
    pub fn new(label: impl Into<Arc<str>>, monogram: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            monogram: monogram.into(),
            image: None,
            size: 40.,
            status: None,
        }
    }
    pub fn image(mut self, image: Arc<stillkit::Image>) -> Self {
        self.image = Some(image);
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(16.);
        self
    }
    pub fn status(mut self, status: impl Into<Arc<str>>) -> Self {
        self.status = Some(status.into());
        self
    }
    pub fn build<S>(self) -> El<S> {
        let mut n = row()
            .size(self.size)
            .radius(self.size / 2.)
            .fill(token::FILL)
            .align(Align::Center)
            .justify(Justify::Center)
            .role(Role::Image)
            .label(self.label);
        n = n.child(if let Some(bitmap) = self.image {
            image(bitmap).size(self.size).radius(self.size / 2.)
        } else {
            text(self.monogram).color(LABEL).font(TextRole::Headline)
        });
        if let Some(status) = self.status {
            n = n.child(
                el().size(10.)
                    .radius(5.)
                    .fill(token::ACCENT)
                    .absolute(self.size - 10., self.size - 10.)
                    .role(Role::Label)
                    .label(status),
            );
        }
        n
    }
}
