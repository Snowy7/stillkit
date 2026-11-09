//! Continuous motion and draw-time rotation, headless: a spinner moves only while it can be
//! seen (not scrolled out, not under a dialog, the window shown, motion not reduced) and leaves
//! no ticker; a rotated element draws, hit-tests and damages turned; arcs start anywhere.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Duration;

use stillkit::el::{Axis, El, Runtime, arc, col, dialog, el, icon};
use stillkit::icons::Icon;
use stillkit::{Op, Rect, Rgba, Scheme, Style, Theme};

const FRAME: Duration = Duration::from_millis(16);

fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

#[derive(Default)]
struct S {
    dialog: bool,
    open: bool,
    presses: usize,
}

/// The arcs' start angles, through clips.
fn starts(ops: &[Op], out: &mut Vec<f32>) {
    for op in ops {
        match op {
            Op::Arc { start, .. } => out.push(*start),
            Op::Clipped { ops, .. } => starts(ops, out),
            Op::Transformed { transform, ops } => { let mut local=Vec::new(); starts(ops,&mut local); out.extend(local.into_iter().map(|a|a+transform.0[1].atan2(transform.0[0]))); },
            _ => {}
        }
    }
}

fn arc_starts(rt: &Runtime<S>) -> Vec<f32> {
    let mut out = Vec::new();
    starts(&rt.list().ops, &mut out);
    out
}

/// A spinner at the top of a scroll view, and a dialog that can open over it.
fn spinner(s: &S) -> El<S> {
    col()
        .pad(10.0)
        .child(
            col().key("list").scroll(Axis::Vertical).h(100.0).child(
                arc(0.25).key("spinner").size(24.0).no_track().spin(Duration::from_secs(1)),
            ).child(el().h(600.0)),
        )
        .overlay(dialog(col().key("panel").w(100.0).h(60.0).fill(stillkit::el::token::CARD)).open(s.dialog))
}

fn settle(rt: &mut Runtime<S>) -> usize {
    for n in 0..200 {
        if !rt.animating() {
            return n;
        }
        rt.frame(FRAME);
    }
    panic!("it never settled");
}

#[test]
fn a_spinner_turns_on_the_clock_without_running_the_view() {
    let mut rt = Runtime::new(S::default(), spinner, theme(), (300, 200), 1.0).unwrap();
    assert!(rt.animating(), "a visible spinner keeps frames coming");
    let before = arc_starts(&rt);
    rt.frame(Duration::from_millis(250));
    let after = arc_starts(&rt);
    assert!((after[0] - before[0] - FRAC_PI_2).abs() < 0.01, "a quarter of a 1 s turn: {before:?} -> {after:?}");
    assert!(rt.damage().is_some_and(|d| d.width() < 40.0), "only the spinner redraws: {:?}", rt.damage());
    assert_eq!(rt.view_runs(), 1);
}

#[test]
fn a_spinner_scrolled_out_of_sight_stops_and_leaves_no_ticker() {
    let mut rt = Runtime::new(S::default(), spinner, theme(), (300, 200), 1.0).unwrap();
    rt.wheel_at(50.0, 50.0, -120 * 3);
    settle(&mut rt);
    assert!(!rt.animating() && rt.wake_at().is_none(), "nothing ticks once it is out of sight");
    let held = arc_starts(&rt);
    assert!(!rt.frame(Duration::from_millis(300)), "nothing is painted");
    assert_eq!(arc_starts(&rt), held, "it holds where it was");
    // Back in sight, it turns again.
    rt.wheel_at(50.0, 50.0, 120 * 3);
    rt.frame(FRAME);
    assert!(rt.animating());
}

#[test]
fn a_spinner_under_a_dialog_stops() {
    let mut rt = Runtime::new(S::default(), spinner, theme(), (300, 200), 1.0).unwrap();
    rt.update(|s| s.dialog = true);
    rt.frame(FRAME);
    settle(&mut rt);
    assert!(!rt.animating() && rt.wake_at().is_none());
    rt.update(|s| s.dialog = false);
    rt.frame(FRAME);
    settle_until_spinning(&mut rt);
}

/// After a dialog's exit, the spinner turns again.
fn settle_until_spinning(rt: &mut Runtime<S>) {
    for _ in 0..100 {
        rt.frame(FRAME);
    }
    assert!(rt.animating(), "it spins once the dialog is gone");
}

#[test]
fn reduced_motion_and_a_hidden_window_hold_a_spinner_still() {
    let mut rt = Runtime::new(S::default(), spinner, theme(), (300, 200), 1.0).unwrap();
    rt.set_reduced_motion(true);
    rt.frame(FRAME);
    assert!(!rt.animating());
    assert_eq!(arc_starts(&rt), [0.0], "at rest at its own angle");
    rt.set_reduced_motion(false);
    rt.frame(FRAME);
    assert!(rt.animating());
    rt.set_visible(false);
    rt.frame(FRAME);
    assert!(!rt.animating() && rt.wake_at().is_none());
    rt.set_visible(true);
    rt.frame(FRAME);
    assert!(rt.animating());
}

/// A wide pressable bar turned a quarter (or not), and a chevron that turns with motion.
fn turned(s: &S) -> El<S> {
    col()
        .pad(40.0)
        .gap(40.0)
        .child(
            el().key("bar")
                .w(80.0)
                .h(20.0)
                .fill(Rgba::new(1.0, 0.0, 0.0, 1.0))
                .rotate(if s.open { FRAC_PI_2 } else { 0.0 })
                .on_press(|s: &mut S| s.presses += 1),
        )
        .child(icon(Icon::ChevronRight).key("chevron").size(20.0).rotate(if s.open { FRAC_PI_2 } else { 0.0 }).animate())
}

#[test]
fn a_rotated_box_draws_hits_and_damages_turned() {
    let mut rt = Runtime::new(S { open: true, ..S::default() }, turned, theme(), (300, 300), 1.0).unwrap();
    let bar = rt.find("bar").unwrap();
    assert_eq!(bar.width(), 80.0, "layout keeps its upright box");
    let c = bar.center();
    let turned = Rect::new(c.x - 10.0, c.y - 40.0, 20.0, 80.0);
    assert!(rt.list().ops.iter().any(|op| matches!(op,Op::Transformed { .. })), "paint uses an affine scope");
    let hit = rt.list().hits.iter().find(|h| h.actionable).unwrap();
    assert!((hit.rect.left-turned.left).abs()<0.001 && (hit.rect.top-turned.top).abs()<0.001, "it hit-tests over the turned box");
    // A press where only the turned box is lands; one where only the upright box was doesn't.
    rt.pointer_down(c.x, c.y - 30.0);
    rt.pointer_up(c.x, c.y - 30.0);
    rt.frame(FRAME);
    rt.pointer_down(c.x + 30.0, c.y);
    rt.pointer_up(c.x + 30.0, c.y);
    rt.frame(FRAME);
    assert_eq!(rt.state().presses, 1);
    let image = rt.snap().unwrap();
    let red = |x: f32, y: f32| image.pixel(x as u32, y as u32)[2] > 200;
    assert!(red(c.x, c.y - 35.0) && !red(c.x + 35.0, c.y), "the pixels are turned");

    // Turning back redraws both its boxes: what it covered and what it covers now.
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    rt.update_instant(|s| s.open = false);
    rt.frame(FRAME);
    let damage = rt.damage().unwrap();
    let both = turned.union(bar);
    assert!(damage.left <= both.left && damage.top <= both.top && damage.right >= both.right && damage.bottom >= both.bottom, "{damage:?} covers {both:?}");
}

#[test]
fn a_turning_chevron_springs_and_keeps_native_geometry() {
    let mut rt = Runtime::new(S::default(), turned, theme(), (300,300),1.0).unwrap();
    rt.update(|s|s.open=true);
    rt.frame(FRAME); rt.frame(FRAME);
    assert!(rt.animating());
    assert!(rt.list().ops.iter().any(|op|matches!(op,Op::Transformed{ops,..} if ops.iter().any(|op|matches!(op,Op::Icon{..})))));
    settle(&mut rt);
    assert!(!rt.animating());
}

#[derive(Default)]
struct Gauge;

#[test]
fn an_arc_starts_at_half_past_seven() {
    let view = |_: &Gauge| -> El<Gauge> {
        col().pad(10.0).child(arc(0.5).key("gauge").size(80.0).stroke(8.0).no_track().arc_angle(1.25 * PI).color(Rgba::new(0.0, 1.0, 0.0, 1.0)))
    };
    let mut rt = Runtime::new(Gauge, view, theme(), (100, 100), 1.0).unwrap();
    let mut out = Vec::new();
    starts(&rt.list().ops, &mut out);
    assert_eq!(out.len(), 1);
    assert!((out[0] - 1.25 * PI).abs() < 1e-6);
    let image = rt.snap().unwrap();
    let r = rt.find("gauge").unwrap();
    let (c, radius) = (r.center(), 36.0);
    // Clockwise angles from twelve: half a turn from 7:30 covers 9 and 12 o'clock and ends at
    // 1:30, missing 3 and 6.
    let green = |turn: f32| {
        let a = turn * TAU;
        let (x, y) = (c.x + radius * a.sin(), c.y - radius * a.cos());
        image.pixel(x as u32, y as u32)[1] > 200
    };
    assert!(green(0.75) && green(0.0) && green(0.70), "9, 12 and just past 7:30");
    assert!(!green(0.25) && !green(0.5) && !green(0.60), "not 3, 6 or just before 7:30");
}
