//! Controlled, nestable two-pane layouts. Keep `SplitState` in the app model.
use crate::*;
use stillkit::el::{KeyCode, PointerPhase};

/// Which dimension a split divides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitAxis {
    /// Panes sit beside each other.
    #[default]
    Horizontal,
    /// Panes sit above each other.
    Vertical,
}

/// Persist `size` with the app's settings; transient drag state stays private.
#[derive(Clone, Debug)]
pub struct SplitState {
    /// Leading pane size in design pixels; zero means collapsed.
    pub size: f32,
    drag: Option<f32>,
}
impl SplitState {
    /// Restore a pane size from app settings.
    pub fn new(size: f32) -> Self {
        Self { size, drag: None }
    }
}

/// A leading pane and a flexible trailing pane, separated by a captured resize grip.
pub struct SplitView<S> {
    key: Arc<str>,
    state: SplitState,
    access: fn(&mut S) -> &mut SplitState,
    first: El<S>,
    second: El<S>,
    axis: SplitAxis,
    min: f32,
    max: f32,
    reset: f32,
    collapse: bool,
}
impl<S: 'static> SplitView<S> {
    /// Both children may themselves be split views. Keys must be unique within the app.
    pub fn new(
        key: impl Into<Arc<str>>,
        state: &SplitState,
        access: fn(&mut S) -> &mut SplitState,
        first: El<S>,
        second: El<S>,
    ) -> Self {
        Self {
            key: key.into(),
            state: state.clone(),
            access,
            first,
            second,
            axis: SplitAxis::Horizontal,
            min: 100.,
            max: 600.,
            reset: 240.,
            collapse: false,
        }
    }
    /// Stack panes vertically instead of horizontally.
    pub fn axis(mut self, axis: SplitAxis) -> Self {
        self.axis = axis;
        self
    }
    /// Leading pane limits; invalid values become a finite, ordered interval.
    pub fn limits(mut self, min: f32, max: f32) -> Self {
        self.min = if min.is_finite() { min.max(0.) } else { 0. };
        self.max = if max.is_finite() {
            max.max(self.min)
        } else {
            self.min.max(600.)
        };
        self
    }
    /// Double-click or Home restores this size.
    pub fn reset_size(mut self, size: f32) -> Self {
        self.reset = size;
        self
    }
    /// Drag below half the minimum or press Enter to collapse/restore the leading edge.
    pub fn collapsible(mut self, yes: bool) -> Self {
        self.collapse = yes;
        self
    }
    fn bounded(value: f32, min: f32, max: f32, collapse: bool) -> f32 {
        let value = if value.is_finite() { value } else { min };
        if collapse && value < min / 2. {
            0.
        } else {
            value.clamp(min, max)
        }
    }
    /// Build the controlled panes and accessible splitter.
    pub fn build(self) -> El<S> {
        let Self {
            key,
            state,
            access,
            first,
            second,
            axis,
            min,
            max,
            reset,
            collapse,
        } = self;
        let reset = Self::bounded(reset, min, max, false);
        let size = Self::bounded(state.size, min, max, collapse);
        let horizontal = axis == SplitAxis::Horizontal;
        let grip = el()
            .key(format!("{key}/splitter"))
            .role(Role::Splitter)
            .label("Resize pane")
            .description("Arrow keys resize. Home resets. Enter collapses or restores.")
            .value(format!("{size:.0} pixels"))
            .range(
                if collapse { 0. } else { min as f64 },
                max as f64,
                10.,
                size as f64,
            )
            .focusable()
            .hovered(|e| e.fill(token::FILL))
            .child(if horizontal {
                el().w(1.).h_full().absolute(2., 0.).fill(token::SEPARATOR)
            } else {
                el().h(1.).w_full().absolute(0., 2.).fill(token::SEPARATOR)
            })
            .cursor(if horizontal {
                stillkit::Cursor::ResizeHorizontal
            } else {
                stillkit::Cursor::ResizeVertical
            })
            .on_pointer(move |s: &mut S, e, tx| {
                if e.button != stillkit::Button::Left {
                    return false;
                }
                let p = access(s);
                match e.phase {
                    PointerPhase::Down => {
                        if e.clicks == 2 {
                            p.size = reset;
                            p.drag = None;
                        } else {
                            p.drag = Some(Self::bounded(p.size, min, max, collapse));
                        }
                    }
                    PointerPhase::Move => {
                        if let Some(start) = p.drag {
                            let travel = e.travel();
                            p.size = Self::bounded(
                                start + if horizontal { travel.x } else { travel.y },
                                min,
                                max,
                                collapse,
                            );
                            tx.instant();
                        }
                    }
                    PointerPhase::Up => p.drag = None,
                    PointerPhase::Cancel => {
                        if let Some(start) = p.drag.take() {
                            p.size = start;
                        }
                    }
                    PointerPhase::Hover => return false,
                }
                true
            })
            .on_key(move |s: &mut S, key, tx| {
                let p = access(s);
                let step = if tx.modifiers().shift { 1. } else { 10. };
                let delta = match (horizontal, key) {
                    (true, KeyCode::LEFT) | (false, KeyCode::UP) => -step,
                    (true, KeyCode::RIGHT) | (false, KeyCode::DOWN) => step,
                    (_, KeyCode::HOME) => {
                        p.size = reset;
                        return true.into();
                    }
                    (_, KeyCode::ENTER) if collapse => {
                        p.size = if p.size == 0. { reset } else { 0. };
                        return true.into();
                    }
                    _ => return false.into(),
                };
                // A collapsed pane opens to its minimum on the first positive step.
                p.size = if p.size == 0. && delta > 0. {
                    min
                } else {
                    Self::bounded(p.size + delta, min, max, collapse)
                };
                true.into()
            })
            .on_step(move |s: &mut S, delta| {
                let p = access(s);
                p.size = Self::bounded(p.size + delta as f32, min, max, collapse);
            });
        let first = if size > 0. {
            col().min_w(0.).min_h(0.).child(first)
        } else {
            col()
        };
        let second = col().min_w(0.).min_h(0.).grow(1.).child(second);
        if horizontal {
            row()
                .key(key)
                .w_full()
                .h_full()
                .child(first.w(size).max_w(size).h_full())
                .child(grip.w(6.).h_full())
                .child(second.w(0.).h_full())
        } else {
            col()
                .key(key)
                .w_full()
                .h_full()
                .child(first.h(size).max_h(size).w_full())
                .child(grip.h(6.).w_full())
                .child(second.h(0.).w_full())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resize_math_bounds_and_collapse_threshold() {
        assert_eq!(SplitView::<()>::bounded(f32::NAN, 100., 400., true), 100.);
        assert_eq!(SplitView::<()>::bounded(49., 100., 400., true), 0.);
        assert_eq!(SplitView::<()>::bounded(50., 100., 400., true), 100.);
        assert_eq!(SplitView::<()>::bounded(900., 100., 400., true), 400.);
        assert_eq!(SplitView::<()>::bounded(-100., 100., 400., false), 100.);
    }
}
