//! The element tree's animation runtime. Views stay pure: every re-layout gives each element
//! new [`Target`]s, and the [`Animator`] springs what it shows (its [`Pose`]) from where it is
//! toward them on its own clock. The view never runs for animation.
//!
//! - Every property is its own spring per component (edges, colour channels), solved exactly
//!   ([`Spring::released`]), so a retarget mid-flight keeps the exact position and velocity,
//!   and a component whose target holds (a stationary edge) never moves.
//! - Edges are relative to the parent's top-left: children ride their parent.
//! - Colours move in premultiplied OKLab without overshoot; ink moves twice as fast as fills,
//!   so text keeps its contrast through a theme switch.
//! - Presence (`Transition`) is one value, 1 shown and 0 away; leaving elements stay drawn
//!   (the runtime keeps them as ghosts) until it settles at 0.
//! - Values on screen are whole device pixels and 8-bit colours, so the slow tail of a spring
//!   repaints identical lists, which the surface does not draw.

use std::collections::{HashMap, HashSet};

use super::element::Transition;
use super::layout::Laid;
use crate::{Point, Rect, Rgba, Spring};

/// Reduced motion's fade for opacity, colour and presence; spatial travel snaps.
pub(crate) const REDUCED_FADE: Spring = Spring::apple(0.15, 0.0);

/// What an element shows once everything settles, from the latest layout (device px).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Target {
    /// Left, top, right, bottom, from the parent's top-left.
    pub rel: [f32; 4],
    pub fill: Option<Rgba>,
    pub fill_gradient: Option<std::sync::Arc<crate::paint::Gradient>>,
    pub border_gradient: Option<std::sync::Arc<crate::paint::Gradient>>,
    /// Its border's colour, and width in device px ([`super::El::border`]).
    pub border: Option<Rgba>,
    pub border_width: f32,
    /// Its inner top and bottom edge colours ([`super::El::edges`]); they snap, not spring.
    pub edges: Option<[Rgba; 2]>,
    /// Text and icon colour.
    pub ink: Rgba,
    pub opacity: f32,
    /// Clamped to a capsule.
    pub radius: f32,
    /// The theme's shadow at this strength (0 or 1).
    pub shadow: f32,
    /// An arc's filled fraction (0 for everything else).
    pub value: f32,
    /// Its turn about its centre, radians ([`super::El::rotate`]).
    pub angle: f32,
    pub transform: Option<std::sync::Arc<[f32; 6]>>,
    /// Its own or an ancestor's spring; `None` lands at once.
    pub motion: Option<Spring>,
    /// What presence moves along: its motion, or the theme's open spring.
    pub open: Spring,
    /// Its slide in device px.
    pub transition: Option<Transition>,
}

/// Maps local coordinates to the screen: `p * s + (x, y)` (presence scale and slide).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Xf {
    pub s: f32,
    pub x: f32,
    pub y: f32,
}

impl Xf {
    pub const ID: Self = Self { s: 1.0, x: 0.0, y: 0.0 };

    /// `self` after `inner`.
    fn then(self, inner: Self) -> Self {
        Self {
            s: self.s * inner.s,
            x: self.s * inner.x + self.x,
            y: self.s * inner.y + self.y,
        }
    }

    fn rect(self, r: Rect) -> Rect {
        Rect {
            left: r.left * self.s + self.x,
            top: r.top * self.s + self.y,
            right: r.right * self.s + self.x,
            bottom: r.bottom * self.s + self.y,
        }
    }
}

/// What an element shows now.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pose {
    /// On screen, whole device pixels.
    pub rect: Rect,
    /// Before presence transforms: where its children hang from.
    pub local: Rect,
    /// Its own and its ancestors' presence transforms.
    pub xf: Xf,
    pub fill: Option<Rgba>,
    pub fill_gradient: Option<std::sync::Arc<crate::paint::Gradient>>,
    pub border_gradient: Option<std::sync::Arc<crate::paint::Gradient>>,
    pub border: Option<Rgba>,
    /// Device px.
    pub border_width: f32,
    pub edges: Option<[Rgba; 2]>,
    pub ink: Rgba,
    /// Its own, presence included; paint multiplies it down the subtree.
    pub opacity: f32,
    /// Local (before `xf.s`).
    pub radius: f32,
    pub shadow: f32,
    /// An arc's filled fraction now.
    pub value: f32,
    /// Its turn about its centre now, radians.
    pub angle: f32,
    pub transform: Option<std::sync::Arc<[f32; 6]>>,
}

impl Pose {
    /// The corner radius on screen, clamped to a capsule.
    pub fn radius(&self) -> f32 {
        (self.radius * self.xf.s)
            .min(self.rect.width().min(self.rect.height()) / 2.0)
            .max(0.0)
    }
}

/// How the changes of a re-layout land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The first frame: nothing animates.
    First,
    /// `Tx::instant`, drags, resizes, a new scale: everything lands at once.
    Instant,
    Animate,
}

/// A value of `N` components springing toward `target`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Prop<const N: usize> {
    pub(crate) target: [f32; N],
    pub(crate) moving: Option<Moving<N>>,
}

/// Where a moving [`Prop`] was let go: its displacement and velocity at `start`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Moving<const N: usize> {
    d0: [f32; N],
    v0: [f32; N],
    start: f64,
    spring: Spring,
}

impl<const N: usize> Prop<N> {
    pub(crate) fn at(target: [f32; N]) -> Self {
        Self {
            target,
            moving: None,
        }
    }

    /// The value and velocity at `t`.
    fn sample(&self, t: f64) -> ([f32; N], [f32; N]) {
        let Some(m) = self.moving else {
            return (self.target, [0.0; N]);
        };
        let dt = (t - m.start).max(0.0) as f32;
        let (mut x, mut v) = ([0.0; N], [0.0; N]);
        for i in 0..N {
            let (d, vel) = m.spring.released(m.d0[i], m.v0[i], dt);
            (x[i], v[i]) = (self.target[i] + d, vel);
        }
        (x, v)
    }

    pub(crate) fn value(&self, t: f64) -> [f32; N] {
        self.sample(t).0
    }

    /// Heads for `target` from where it is at `t`, with its velocity, along `spring`; with no
    /// spring, lands on it. An unchanged target keeps its flight.
    pub(crate) fn set(&mut self, target: [f32; N], spring: Option<Spring>, t: f64) {
        if target == self.target {
            return;
        }
        let (x, v) = self.sample(t);
        self.target = target;
        self.moving = spring.map(|spring| Moving {
            d0: std::array::from_fn(|i| x[i] - target[i]),
            v0: v,
            start: t,
            spring,
        });
        if self.moving.is_some_and(|m| m.d0.iter().chain(&m.v0).all(|&c| c == 0.0)) {
            self.moving = None;
        }
    }

    /// Lands it once every component is within `eps` of its target and slower than `10 eps`
    /// per second. Returns whether it still moves.
    pub(crate) fn settle(&mut self, t: f64, eps: f32) -> bool {
        let Some(m) = self.moving else { return false };
        let (x, v) = self.sample(t);
        let rest = (0..N).all(|i| (x[i] - self.target[i]).abs() < eps && v[i].abs() < eps * 10.0);
        if rest || t - m.start > 10.0 {
            self.moving = None;
        }
        self.moving.is_some()
    }
}

/// One element's animated properties.
#[derive(Debug, Clone)]
struct Track {
    rect: Prop<4>,
    fill_gradient: Option<Box<GradientTrack>>,
    border_gradient: Option<Box<GradientTrack>>,
    /// Premultiplied OKLab, and the exact colour it rests on.
    fill: Prop<4>,
    fill_rest: Option<Rgba>,
    border: Prop<4>,
    border_rest: Option<Rgba>,
    border_width: f32,
    edges: Option<[Rgba; 2]>,
    ink: Prop<4>,
    ink_rest: Rgba,
    opacity: Prop<1>,
    radius: Prop<1>,
    shadow: Prop<1>,
    value: Prop<1>,
    angle: Prop<1>,
    transform: Option<Box<Prop<6>>>,
    /// 1 shown, 0 away.
    presence: Prop<1>,
    transition: Option<Transition>,
    open: Spring,
    leaving: bool,
}

impl Track {
    fn at(target: &Target) -> Self {
        Self {
            rect: Prop::at(target.rel),
            fill_gradient: target.fill_gradient.as_deref().map(|g| Box::new(GradientTrack::at(g))),
            border_gradient: target.border_gradient.as_deref().map(|g| Box::new(GradientTrack::at(g))),
            fill: Prop::at(oklab(target.fill)),
            fill_rest: target.fill,
            border: Prop::at(oklab(target.border)),
            border_rest: target.border,
            border_width: target.border_width,
            edges: target.edges,
            ink: Prop::at(oklab(Some(target.ink))),
            ink_rest: target.ink,
            opacity: Prop::at([target.opacity]),
            radius: Prop::at([target.radius]),
            shadow: Prop::at([target.shadow]),
            value: Prop::at([target.value]),
            angle: Prop::at([target.angle]),
            transform: target.transform.as_deref().map(|v|Box::new(Prop::at(*v))),
            presence: Prop::at([1.0]),
            transition: target.transition,
            open: target.open,
            leaving: false,
        }
    }

    fn retarget(&mut self, target: &Target, mode: Mode, reduced: bool, t: f64) {
        let motion = target.motion.filter(|_| mode == Mode::Animate);
        let spatial = motion.filter(|_| !reduced);
        let fade = motion.map(|m| if reduced { REDUCED_FADE } else { m.calm() });
        self.rect.set(target.rel, spatial, t);
        gradient_retarget(&mut self.fill_gradient, target.fill_gradient.as_deref(), fade, t);
        gradient_retarget(&mut self.border_gradient, target.border_gradient.as_deref(), fade, t);
        self.fill.set(oklab(target.fill), fade, t);
        self.fill_rest = target.fill;
        self.border.set(oklab(target.border), fade, t);
        self.border_rest = target.border;
        self.border_width = target.border_width;
        self.edges = target.edges;
        self.ink.set(oklab(Some(target.ink)), fade.map(|s| s.faster(2.0)), t);
        self.ink_rest = target.ink;
        self.opacity.set([target.opacity], fade, t);
        self.radius.set([target.radius], spatial, t);
        self.shadow.set([target.shadow], fade, t);
        self.value.set([target.value], spatial, t);
        self.angle.set([target.angle], spatial, t);
        if self.transform.is_some() || target.transform.is_some() {
            self.transform.get_or_insert_with(||Box::new(Prop::at([0.,0.,1.,1.,0.5,0.5]))).set(target.transform.as_deref().copied().unwrap_or([0.,0.,1.,1.,0.5,0.5]),spatial,t);
        }
        self.transition = target.transition;
        self.open = target.open;
        if self.leaving {
            self.leaving = false;
            let spring = (mode == Mode::Animate).then(|| self.presence_spring(reduced));
            self.presence.set([1.0], spring, t);
        }
    }

    fn presence_spring(&self, reduced: bool) -> Spring {
        if reduced { REDUCED_FADE } else { self.open }
    }

    /// Lands what has settled by `t`; whether anything still moves.
    fn settle(&mut self, t: f64) -> bool {
        let color = 1.0 / 1024.0;
        // Every prop settles, so no short-circuiting.
        [
            self.rect.settle(t, 0.1),
            self.fill_gradient.as_mut().is_some_and(|g| g.settle(t)),
            self.border_gradient.as_mut().is_some_and(|g| g.settle(t)),
            self.fill.settle(t, color),
            self.border.settle(t, color),
            self.ink.settle(t, color),
            self.opacity.settle(t, color),
            self.radius.settle(t, 0.1),
            self.shadow.settle(t, color),
            self.value.settle(t, 1.0 / 2048.0),
            self.angle.settle(t, 1.0 / 1024.0),
            self.transform.as_mut().is_some_and(|p|p.settle(t,0.001)),
            self.presence.settle(t, color),
        ]
        .contains(&true)
    }

    fn moving(&self) -> bool {
        self.fill_gradient.as_ref().is_some_and(|g| g.moving())
            || self.border_gradient.as_ref().is_some_and(|g| g.moving())
            || self.rect.moving.is_some()
            || self.fill.moving.is_some()
            || self.border.moving.is_some()
            || self.ink.moving.is_some()
            || self.opacity.moving.is_some()
            || self.radius.moving.is_some()
            || self.shadow.moving.is_some()
            || self.value.moving.is_some()
            || self.angle.moving.is_some()
            || self.transform.as_ref().is_some_and(|p|p.moving.is_some())
            || self.presence.moving.is_some()
    }
}

/// Every animated element's track, by identity, and the animation clock.
#[derive(Debug, Default)]
pub(crate) struct Animator {
    tracks: HashMap<u64, Track>,
    /// Seconds: the runtime's clock times its time scale.
    pub t: f64,
    /// Spatial travel snaps; opacity, colour and presence take [`REDUCED_FADE`].
    pub reduced: bool,
}

impl Animator {
    /// Brings the tracks to a new layout's targets. `prev` holds the identities laid out
    /// before: an element that is new while its parent is not enters by its transition.
    pub fn retarget(&mut self, laid: &[Laid], targets: &[Target], prev: &HashSet<u64>, mode: Mode) {
        let (t, reduced) = (self.t, self.reduced);
        let mut fresh = vec![false; laid.len()];
        for (i, (l, target)) in laid.iter().zip(targets).enumerate() {
            fresh[i] = !prev.contains(&l.id);
            if let Some(track) = self.tracks.get_mut(&l.id) {
                track.retarget(target, mode, reduced, t);
                continue;
            }
            if target.motion.is_none() && target.transition.is_none() {
                continue;
            }
            let mut track = Track::at(target);
            let enters = mode == Mode::Animate
                && fresh[i]
                && target.transition.is_some()
                && l.parent.is_some_and(|p| !fresh[p]);
            if enters {
                track.presence = Prop::at([0.0]);
                track.presence.set([1.0], Some(track.presence_spring(reduced)), t);
            }
            self.tracks.insert(l.id, track);
        }
    }

    /// Starts element `id`'s exit, if it has a transition to exit by.
    pub fn leave(&mut self, id: u64) -> bool {
        let (t, reduced) = (self.t, self.reduced);
        let Some(track) = self.tracks.get_mut(&id).filter(|tr| tr.transition.is_some()) else {
            return false;
        };
        track.leaving = true;
        let spring = track.presence_spring(reduced);
        track.presence.set([0.0], Some(spring), t);
        true
    }

    /// Forgets the tracks of elements that are gone.
    pub fn retain(&mut self, keep: impl Fn(u64) -> bool) {
        self.tracks.retain(|&id, _| keep(id));
    }

    /// Lands everything that settled by now. Returns the leaving elements whose exit ended
    /// (their tracks are gone).
    pub fn settle(&mut self) -> Vec<u64> {
        let t = self.t;
        let mut left = Vec::new();
        for (&id, track) in &mut self.tracks {
            if !track.settle(t) && track.leaving {
                left.push(id);
            }
        }
        for id in &left {
            self.tracks.remove(id);
        }
        left
    }

    /// Whether anything moves (the host keeps frames coming while it does).
    pub fn moving(&self) -> bool {
        self.tracks.values().any(Track::moving)
    }

    /// What every laid-out element shows now, in the same order. `scrolled` gives each scroll
    /// view's offset (by its place in `laid`, whole device px): its children move back by it.
    pub fn poses(&self, laid: &[Laid], targets: &[Target], scrolled: &HashMap<usize, Point>) -> Vec<Pose> {
        let mut out: Vec<Pose> = Vec::with_capacity(laid.len());
        for (l, target) in laid.iter().zip(targets) {
            let parent = l.parent.and_then(|p| out.get(p).map(|pose| (p, pose)));
            let (origin, xf) = parent.map_or((Point::new(0.0, 0.0), Xf::ID), |(p, pose)| {
                let by = scrolled.get(&p).copied().unwrap_or(Point::new(0.0, 0.0));
                (Point::new(pose.local.left - by.x, pose.local.top - by.y), pose.xf)
            });
            let pose = match self.tracks.get(&l.id) {
                Some(track) => self.pose(track, origin, xf),
                None => Pose {
                    rect: Rect::default(),
                    local: offset(target.rel, origin),
                    xf,
                    fill: target.fill,
                    fill_gradient: target.fill_gradient.clone(),
                    border_gradient: target.border_gradient.clone(),
                    border: target.border,
                    border_width: target.border_width,
                    edges: target.edges,
                    ink: target.ink,
                    opacity: target.opacity,
                    radius: target.radius,
                    shadow: target.shadow,
                    value: target.value,
                    angle: target.angle,
                    transform: target.transform.clone(),
                },
            };
            out.push(Pose {
                rect: round(pose.xf.rect(pose.local)),
                ..pose
            });
        }
        out
    }

    /// A tracked element's pose before rounding: its local rect hangs from `origin`.
    fn pose(&self, track: &Track, origin: Point, parent: Xf) -> Pose {
        let t = self.t;
        let local = offset(track.rect.value(t), origin);
        let (xf, fade) = self.presence(track, local, parent);
        let color = |prop: &Prop<4>, rest: Option<Rgba>| match prop.moving {
            Some(_) => Some(rgba(prop.value(t))),
            None => rest,
        };
        let scalar = |prop: &Prop<1>| match prop.moving {
            Some(_) => quantize(prop.value(t)[0].clamp(0.0, 1.0)),
            None => prop.target[0],
        };
        Pose {
            rect: Rect::default(),
            local,
            xf,
            fill: color(&track.fill, track.fill_rest),
            fill_gradient: track.fill_gradient.as_ref().map(|g| std::sync::Arc::new(g.sample(t))),
            border_gradient: track.border_gradient.as_ref().map(|g| std::sync::Arc::new(g.sample(t))),
            border: color(&track.border, track.border_rest),
            border_width: track.border_width,
            edges: track.edges,
            ink: color(&track.ink, Some(track.ink_rest)).unwrap_or(track.ink_rest),
            opacity: scalar(&track.opacity) * fade,
            radius: track.radius.value(t)[0].max(0.0),
            shadow: scalar(&track.shadow),
            // An arc's sweep to a thousandth of a turn, so its slow tail repaints nothing new.
            value: match track.value.moving {
                Some(_) => (track.value.value(t)[0].clamp(0.0, 1.0) * 1000.0).round() / 1000.0,
                None => track.value.target[0],
            },
            // To a thousandth of a radian, so a slow tail repaints nothing new.
            transform: track.transform.as_ref().map(|p|std::sync::Arc::new(p.value(t))),
            angle: match track.angle.moving {
                Some(_) => (track.angle.value(t)[0] * 1000.0).round() / 1000.0,
                None => track.angle.target[0],
            },
        }
    }

    /// A ghost's poses now: `frozen` is its subtree as last shown, local rects relative to
    /// its parent's, which shows `parent` now. The whole subtree takes the ghost's presence.
    pub fn ghost(&self, id: u64, frozen: &[Pose], parent: &Pose) -> Vec<Pose> {
        let (Some(track), Some(root)) = (self.tracks.get(&id), frozen.first()) else {
            return Vec::new();
        };
        let origin = Point::new(parent.local.left, parent.local.top);
        // It keeps the look it left with; only its presence moves.
        let (xf, fade) = self.presence(track, root.local.offset(origin.x, origin.y), parent.xf);
        frozen
            .iter()
            .enumerate()
            .map(|(j, p)| {
                let local = p.local.offset(origin.x, origin.y);
                Pose {
                    rect: round(xf.rect(local)),
                    local,
                    xf,
                    opacity: if j == 0 { p.opacity * fade } else { p.opacity },
                    ..p.clone()
                }
            })
            .collect()
    }

    /// The transform and fade presence gives an element whose local rect is `local`.
    fn presence(&self, track: &Track, local: Rect, parent: Xf) -> (Xf, f32) {
        let presence = track.presence.value(self.t)[0];
        let Some(tr) = track.transition.filter(|_| presence != 1.0) else {
            return (parent, 1.0);
        };
        let away = 1.0 - presence;
        let fade = quantize((1.0 - (1.0 - tr.opacity) * away).clamp(0.0, 1.0));
        if self.reduced {
            return (parent, fade);
        }
        let k = (1.0 + (tr.scale - 1.0) * away).max(0.0);
        let c = local.center();
        let own = Xf {
            s: k,
            x: c.x * (1.0 - k) + tr.slide.0 * away,
            y: c.y * (1.0 - k) + tr.slide.1 * away,
        };
        (parent.then(own), fade)
    }
}

fn offset([l, t, r, b]: [f32; 4], o: Point) -> Rect {
    Rect {
        left: o.x + l,
        top: o.y + t,
        right: o.x + r,
        bottom: o.y + b,
    }
}

fn round(r: Rect) -> Rect {
    Rect {
        left: r.left.round(),
        top: r.top.round(),
        right: r.right.round(),
        bottom: r.bottom.round(),
    }
}

/// To the nearest 8-bit step, so the tail of a fade repaints identical lists.
fn quantize(v: f32) -> f32 {
    (v * 255.0).round() / 255.0
}

/// Premultiplied OKLab (L·a, a·a, b·a, alpha) of an sRGB colour; `None` is clear.
pub(crate) fn oklab(c: Option<Rgba>) -> [f32; 4] {
    let Some(c) = c else { return [0.0; 4] };
    let lin = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(c.r), lin(c.g), lin(c.b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let a = c.a;
    [
        (0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s) * a,
        (1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s) * a,
        (0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s) * a,
        a,
    ]
}

/// Back from [`oklab`], clamped to sRGB and to 8-bit steps.
pub(crate) fn rgba([pl, pa, pb, a]: [f32; 4]) -> Rgba {
    let a = a.clamp(0.0, 1.0);
    if a < 1.0 / 512.0 {
        return Rgba::TRANSPARENT;
    }
    let (ll, aa, bb) = (pl / a, pa / a, pb / a);
    let l = (ll + 0.396_337_78 * aa + 0.215_803_76 * bb).powi(3);
    let m = (ll - 0.105_561_346 * aa - 0.063_854_17 * bb).powi(3);
    let s = (ll - 0.089_484_18 * aa - 1.291_485_5 * bb).powi(3);
    let srgb = |v: f32| {
        let v = v.max(0.0);
        let e = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        quantize(e.clamp(0.0, 1.0))
    };
    Rgba::new(
        srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        quantize(a),
    )
}

/// Native stops spring independently; changing kind or stop count starts a new topology.
#[derive(Debug, Clone)]
struct GradientTrack {
    rest: crate::paint::Gradient,
    stops: Vec<(Prop<1>, Prop<4>)>,
    angle: Prop<1>,
}
impl GradientTrack {
    fn at(g: &crate::paint::Gradient) -> Self {
        Self { rest: g.clone(), stops: g.stops.iter().map(|s| (Prop::at([s.offset]), Prop::at(oklab(Some(s.color))))).collect(), angle: Prop::at([g.angle.unwrap_or(0.0)]) }
    }
    fn moving(&self) -> bool {
        self.angle.moving.is_some() || self.stops.iter().any(|(p,c)| p.moving.is_some() || c.moving.is_some())
    }
    fn sample(&self, t: f64) -> crate::paint::Gradient {
        if !self.moving() { return self.rest.clone(); }
        let stops: Vec<_> = self.stops.iter().map(|(p,c)| crate::paint::GradientStop { offset: p.value(t)[0], color: rgba(c.value(t)) }).collect();
        crate::paint::Gradient::new(&stops, self.rest.angle.map(|_| self.angle.value(t)[0]))
    }
    fn settle(&mut self, t: f64) -> bool {
        let mut moving = self.angle.settle(t, 0.001);
        for (p,c) in &mut self.stops { moving |= p.settle(t, 0.001); moving |= c.settle(t, 1.0 / 1024.0); }
        moving
    }
}
fn gradient_retarget(track: &mut Option<Box<GradientTrack>>, target: Option<&crate::paint::Gradient>, spring: Option<Spring>, t: f64) {
    let Some(g) = target else { *track = None; return };
    if let Some(tr) = track.as_mut().filter(|tr| tr.stops.len() == g.stops.len() && tr.rest.angle.is_some() == g.angle.is_some()) {
        for ((p,c), stop) in tr.stops.iter_mut().zip(g.stops.iter()) {
            p.set([stop.offset], spring, t);
            c.set(oklab(Some(stop.color)), spring, t);
        }
        tr.angle.set([g.angle.unwrap_or(0.0)], spring, t);
        tr.rest = g.clone();
    } else { *track = Some(Box::new(GradientTrack::at(g))); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_round_trips_and_blends_premultiplied() {
        for c in [
            Rgba::new(1.0, 0.0, 0.0, 1.0),
            Rgba::rgb8([10, 132, 255], 1.0),
            Rgba::rgb8([200, 200, 200], 0.5),
        ] {
            let back = rgba(oklab(Some(c)));
            for (x, y) in [(c.r, back.r), (c.g, back.g), (c.b, back.b), (c.a, back.a)] {
                assert!((x - y).abs() <= 1.0 / 255.0, "{c:?} -> {back:?}");
            }
        }
        // Fading a colour in from clear keeps its hue: halfway is the colour at half alpha.
        let blue = oklab(Some(Rgba::rgb8([10, 132, 255], 1.0)));
        let half = rgba(std::array::from_fn(|i| blue[i] * 0.5));
        assert!((half.a - 0.5).abs() < 0.01 && (half.b - 1.0).abs() < 0.01, "{half:?}");
    }

    #[test]
    fn a_retarget_keeps_position_and_velocity() {
        let spring = Spring::SNAPPY;
        let mut p = Prop::at([0.0]);
        p.set([100.0], Some(spring), 0.0);
        let ([x], [v]) = p.sample(0.1);
        p.set([-50.0], Some(spring), 0.1);
        let (x2, v2) = p.sample(0.1);
        assert!((x - x2[0]).abs() < 1e-3 && (v - v2[0]).abs() < 1e-2, "{x:?} {v:?} -> {x2:?} {v2:?}");
        // It goes on the same way for a while, then turns toward the new target.
        assert!(p.sample(0.11).0[0] > x2[0]);
        assert!(p.settle(0.5, 0.1));
        assert!(!p.settle(5.0, 0.1));
        assert_eq!(p.value(5.0), [-50.0]);
    }
}
