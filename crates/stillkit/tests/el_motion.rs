//! The element tree's animation, headless: nothing moves on the first frame, retargets keep
//! velocity, exits stay drawn but take no hits, reduced motion, instant changes, the ticker's
//! end, tight damage, and golden frame strips through the `--script` + `--at` path.

#[allow(dead_code)]
#[path = "../examples/el_hello.rs"]
mod hello;
#[allow(dead_code)]
#[path = "../examples/el_motion.rs"]
mod motion;

use std::path::Path;
use std::time::Duration;

use motion::{Motion, theme, view};
use stillkit::el::{El, Runtime, Script, Transition, col, el};
use stillkit::{Image, Op, Rect, Spring};

const FRAME: Duration = Duration::from_millis(16);

fn runtime() -> Runtime<Motion> {
    Runtime::new(Motion::default(), view, theme(true), (340, 260), 1.0).unwrap()
}

fn hello() -> Runtime<hello::Hello> {
    Runtime::new(hello::Hello { on: false }, hello::view, hello::theme(), (320, 180), 1.0).unwrap()
}

/// Frames of `dt` until nothing moves; how long that took.
fn settle<S>(rt: &mut Runtime<S>, dt: Duration) -> Duration {
    let start = rt.now();
    while rt.animating() {
        rt.frame(dt);
        assert!(rt.now() - start < Duration::from_secs(5), "never settled");
    }
    rt.now() - start
}

fn draws_text<S>(rt: &Runtime<S>, s: &str) -> bool {
    rt.list().ops.iter().any(|op| matches!(op, Op::Text { text, .. } if &**text == s))
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.left >= outer.left && inner.top >= outer.top && inner.right <= outer.right && inner.bottom <= outer.bottom
}

#[test]
fn nothing_animates_on_the_first_frame() {
    let mut rt = runtime();
    assert!(!rt.animating(), "rows with transitions are simply there");
    assert_eq!(rt.presented("row-1").unwrap().opacity, 1.0);
    assert_eq!(rt.presented("card").unwrap().rect, rt.find("card").unwrap());
    let paints = rt.paints();
    assert!(!rt.frame(Duration::from_secs(1)));
    assert_eq!(rt.paints(), paints);
}

#[test]
fn the_ticker_disarms_once_everything_settles() {
    let mut rt = runtime();
    assert!(rt.click("add"));
    rt.frame(FRAME);
    assert!(rt.animating(), "the new row enters and the rows below make room");
    let took = settle(&mut rt, FRAME);
    assert!(took < Duration::from_millis(1500), "{took:?}");
    let (paints, views) = (rt.paints(), rt.view_runs());
    for _ in 0..30 {
        assert!(!rt.frame(FRAME), "no frame paints once it settled");
    }
    assert_eq!((rt.paints(), rt.view_runs()), (paints, views));
    assert_eq!(rt.view_runs(), 2, "the view ran once for the change, never for its animation");
}

/// A dot 1000 px from home, so it is fast mid-flight.
struct Far {
    right: bool,
}

fn far(s: &Far) -> El<Far> {
    col().child(
        el().key("dot")
            .size(10.0)
            .fill(stillkit::Rgba::new(1.0, 1.0, 1.0, 1.0))
            .motion(Spring::SNAPPY)
            .absolute(if s.right { 1000.0 } else { 0.0 }, 0.0),
    )
}

#[test]
fn a_retarget_mid_flight_keeps_position_and_velocity() {
    let theme = hello::theme();
    let mut rt = Runtime::new(Far { right: false }, far, theme, (1100, 20), 1.0).unwrap();
    rt.update(|s| s.right = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(80));
    let before = rt.presented("dot").unwrap().rect.left;
    assert!(before > 50.0 && before < 950.0, "mid-flight: {before}");
    rt.update(|s| s.right = false);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("dot").unwrap().rect.left, before, "no jump when it turns back");
    rt.frame(FRAME);
    let after = rt.presented("dot").unwrap().rect.left;
    assert!(after > before, "it carries its velocity on before turning: {before} -> {after}");
    settle(&mut rt, FRAME);
    assert_eq!(rt.presented("dot").unwrap().rect.left, 0.0);
}

#[test]
fn layout_moves_per_edge_and_children_ride_along() {
    let mut rt = runtime();
    let card = rt.find("card").unwrap();
    assert!(rt.click("resize"));
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(60));
    let now = rt.presented("card").unwrap().rect;
    let end = rt.find("card").unwrap();
    assert_eq!((now.left, now.top), (card.left, card.top), "stationary edges never move");
    assert!(now.right > card.right && now.right < end.right, "{now:?}");
    assert!(now.bottom < card.bottom && now.bottom > end.bottom, "{now:?}");
}

#[test]
fn exits_stay_drawn_until_settled_and_take_no_hits() {
    let mut rt = runtime();
    let gone = rt.find("row-2").unwrap();
    assert!(rt.click("row-2"));
    rt.frame(FRAME);
    assert!(rt.presented("row-2").is_none(), "it left the view");
    assert!(draws_text(&rt, "Row 2"), "but is still drawn on its way out");
    assert!(rt.list().hits.iter().all(|h| &*h.label != "Row 2"), "and takes no hits");
    // The row below moves up into its place while it fades.
    rt.frame(FRAME * 4);
    assert!(draws_text(&rt, "Row 2"));
    let below = rt.presented("row-3").unwrap().rect;
    assert!(below.top > gone.top && below.top < gone.bottom + 6.0, "{below:?}");
    settle(&mut rt, FRAME);
    assert!(!draws_text(&rt, "Row 2"), "gone once its exit settled");
    assert_eq!(rt.find("row-3").unwrap().top, gone.top);
}

struct Shown {
    on: bool,
}

fn shown(s: &Shown) -> El<Shown> {
    col().child(if s.on {
        el().key("toast").size(40.0).fill(stillkit::Rgba::new(1.0, 1.0, 1.0, 1.0)).transition(Transition::scale(0.8))
    } else {
        el().key("other").size(1.0)
    })
}

#[test]
fn coming_back_mid_exit_reverses_smoothly() {
    let mut rt = Runtime::new(Shown { on: true }, shown, hello::theme(), (60, 60), 1.0).unwrap();
    rt.update(|s| s.on = false);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(100));
    let leaving = rt.list().ops.iter().find_map(|op| match op {
        Op::Fill { rect, color, .. } if color.a > 0.0 => Some((*rect, color.a)),
        _ => None,
    });
    let (rect, alpha) = leaving.expect("still drawn mid-exit");
    assert!(alpha < 1.0 && rect.width() < 40.0, "fading and shrinking: {alpha} {rect:?}");
    rt.update(|s| s.on = true);
    rt.frame(Duration::ZERO);
    let back = rt.presented("toast").unwrap();
    assert!((back.opacity - alpha).abs() < 0.02, "picks up where it was: {} vs {alpha}", back.opacity);
    assert_eq!(back.rect, rect);
    rt.frame(FRAME);
    settle(&mut rt, FRAME);
    let back = rt.presented("toast").unwrap();
    assert_eq!((back.opacity, back.rect.width()), (1.0, 40.0));
}

#[test]
fn reduced_motion_snaps_travel_and_fades_colour() {
    let mut rt = runtime();
    rt.set_reduced_motion(true);
    assert!(rt.click("resize"));
    rt.frame(FRAME);
    assert_eq!(rt.presented("card").unwrap().rect, rt.find("card").unwrap(), "travel snaps");
    assert!(!rt.animating());

    let dark = rt.presented("row-1").unwrap().fill.unwrap();
    assert!(rt.click("theme"));
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(40));
    let mid = rt.presented("row-1").unwrap().fill.unwrap();
    let light = theme(false).color.card;
    assert!(mid != dark && mid != light, "colour takes a short fade: {mid:?}");
    let took = settle(&mut rt, FRAME);
    assert!(took < Duration::from_millis(400), "{took:?}");
    assert_eq!(rt.presented("row-1").unwrap().fill, Some(light));
}

#[test]
fn instant_changes_land_at_once_and_the_time_scale_slows_the_rest() {
    let mut rt = hello();
    rt.update_instant(|s| s.on = true);
    rt.frame(Duration::ZERO);
    assert!(!rt.animating());
    assert_eq!(rt.presented("knob").unwrap().rect, rt.find("knob").unwrap());

    rt.update(|s| s.on = false);
    rt.frame(Duration::ZERO);
    let real = settle(&mut rt, Duration::from_millis(1));
    rt.set_time_scale(0.5);
    rt.update(|s| s.on = true);
    rt.frame(Duration::ZERO);
    let slow = settle(&mut rt, Duration::from_millis(1));
    let ratio = slow.as_secs_f32() / real.as_secs_f32();
    assert!((1.8..2.2).contains(&ratio), "{real:?} -> {slow:?}");
}

#[test]
fn damage_during_an_animation_covers_only_what_moves() {
    let mut rt = hello();
    assert!(rt.click("switch"));
    rt.frame(FRAME); // the label's text changes in this one, and the knob sets off
    let track = rt.find("switch").unwrap().inflate(1.0);
    let mut frames = 0;
    while rt.animating() {
        if rt.frame(FRAME) {
            let damage = rt.damage();
            assert!(damage.is_none_or(|d| inside(d, track)), "{damage:?} outside {track:?}");
            frames += usize::from(damage.is_some());
        }
    }
    assert!(frames > 5, "the knob moved over several frames: {frames}");

    // A card changing size redraws the card and its shadow, not the list beside it (beyond
    // where the shadow falls).
    let mut rt = runtime();
    let before = rt.find("card").unwrap();
    assert!(rt.click("resize"));
    rt.frame(FRAME);
    let e = rt.theme().elevation;
    let reach = before.union(rt.find("card").unwrap()).inflate(e.spread + e.drop + 1.0);
    while rt.animating() {
        rt.frame(FRAME);
        let damage = rt.damage();
        assert!(damage.is_none_or(|d| inside(d, reach)), "{damage:?} outside {reach:?}");
    }
}

// ---- Golden frame strips --------------------------------------------------------------------

/// The frame at `at` seconds, made the way `app --script .. --at .. --snap ..` makes it.
fn frame_at(script: &Script, at: f64, reduced: bool) -> Image {
    let mut rt = runtime();
    rt.set_reduced_motion(reduced);
    rt.play(script, Some(Duration::from_secs_f64(at)));
    rt.frame(Duration::ZERO);
    rt.snap().unwrap()
}

/// Frames side by side.
fn strip(frames: &[Image]) -> Image {
    let (w, h) = (frames[0].width, frames[0].height);
    let mut pixels = vec![0; (w * frames.len() as u32 * h * 4) as usize];
    let stride = (w * frames.len() as u32 * 4) as usize;
    for (n, frame) in frames.iter().enumerate() {
        for y in 0..h as usize {
            let row = &frame.pixels[y * w as usize * 4..][..w as usize * 4];
            pixels[y * stride + n * w as usize * 4..][..row.len()].copy_from_slice(row);
        }
    }
    Image {
        width: w * frames.len() as u32,
        height: h,
        pixels,
    }
}

fn check_strip(name: &str, click: &str, reduced: bool) {
    let script = Script::parse(&format!(r#"[{{"at": 0.1, "click": "{click}"}}]"#)).unwrap();
    let frames: Vec<Image> = [0.1, 0.15, 0.2, 0.3, 0.45, 1.5]
        .iter()
        .map(|&at| frame_at(&script, at, reduced))
        .collect();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/el-motion-{name}.png"));
    stillkit::golden::check(&strip(&frames), &path).unwrap();
}

#[test]
fn golden_strip_a_row_enters() {
    check_strip("add", "add", false);
}

#[test]
fn golden_strip_a_row_exits() {
    check_strip("remove", "row-2", false);
}

#[test]
fn golden_strip_the_card_resizes() {
    check_strip("resize", "resize", false);
}

#[test]
fn golden_strip_the_theme_switches() {
    check_strip("theme", "theme", false);
}

#[test]
fn golden_strip_a_row_exits_with_reduced_motion() {
    check_strip("remove-reduced", "row-2", true);
}
