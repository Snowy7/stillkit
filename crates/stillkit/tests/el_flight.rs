//! Shared geometry across unrelated parents, interruption and reduced motion.
use std::time::Duration;
use stillkit::el::{self, El, Runtime};
use stillkit::{Rgba, Scheme, Theme};
fn view(right: &bool) -> El<bool> {
    let card = el::el()
        .key("card")
        .flight("hero")
        .on_press(|s: &mut bool| *s = !*s)
        .w(if *right { 100. } else { 40. })
        .h(40.)
        .radius(if *right { 20. } else { 2. })
        .fill(if *right {
            Rgba::new(0., 0., 1., 1.)
        } else {
            Rgba::new(1., 0., 0., 1.)
        })
        .opacity(if *right { 0.5 } else { 1. });
    let mut a = el::el().key("left").w(100.).h(100.);
    let mut b = el::el().key("right").w(140.).h(100.);
    if *right {
        b = b.child(card);
    } else {
        a = a.child(card);
    }
    el::row().child(a).child(b)
}
fn runtime() -> Runtime<bool> {
    Runtime::new(
        false,
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (300, 160),
        1.,
    )
    .unwrap()
}
#[test]
fn cross_parent_flight_carries_geometry_and_style_then_goes_idle() {
    let mut rt = runtime();
    let start = rt.presented("card").unwrap();
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("card").unwrap().rect, start.rect);
    assert_eq!(rt.presented("card").unwrap().fill, start.fill);
    rt.frame(Duration::from_millis(80));
    let middle = rt.presented("card").unwrap();
    assert!(middle.rect.left > start.rect.left && middle.rect.left < 100.);
    assert_ne!(middle.fill, start.fill);
    assert!(middle.opacity < 1.0 && middle.opacity > 0.5);
    let radius = rt
        .list()
        .ops
        .iter()
        .find_map(|op| match op {
            stillkit::Op::Fill { radius, .. } => Some(*radius),
            _ => None,
        })
        .unwrap();
    assert!(radius > 2.0 && radius < 20.0);
    let views = rt.view_runs();
    for _ in 0..200 {
        rt.frame(Duration::from_millis(16));
    }
    assert_eq!(rt.presented("card").unwrap().rect, rt.find("card").unwrap());
    assert!(!rt.animating());
    assert_eq!(views, rt.view_runs());
}
#[test]
fn interruption_preserves_the_current_rectangle() {
    let mut rt = runtime();
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(80));
    let rect = rt.presented("card").unwrap().rect;
    assert!(rt.click("card"));
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("card").unwrap().rect, rect);
    rt.frame(Duration::from_millis(8));
    assert!(rt.presented("card").unwrap().rect.left > rect.left);
}
#[test]
fn reduced_motion_and_instant_updates_snap_travel() {
    let mut rt = runtime();
    rt.set_reduced_motion(true);
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("card").unwrap().rect, rt.find("card").unwrap());
    rt.update_instant(|s| *s = false);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("card").unwrap().rect, rt.find("card").unwrap());
}

#[test]
#[cfg(all(windows, feature = "golden"))]
fn flight_pixels_between_parents() {
    let mut rt = runtime();
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(80));
    stillkit::golden::check(
        &rt.snap().unwrap(),
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/el-flight.png"),
    )
    .unwrap();
}
#[test]
fn ambiguous_keys_do_not_choose_a_source() {
    let mut rt = Runtime::new(false, |right| {
        let mut root = el::row();
        if !right {
            root = root.child(el::el().key("a").flight("same").size(20.))
                .child(el::el().key("b").flight("same").size(30.));
        } else {
            root = root.child(el::el().key("c").flight("same").size(40.));
        }
        root
    }, Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE), (300,160),1.).unwrap();
    rt.update(|s| *s = true); rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("c").unwrap().rect, rt.find("c").unwrap());
    assert!(!rt.animating());
}

#[test]
fn enabling_reduced_motion_lands_an_active_flight() {
    let mut rt = runtime();
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(80));
    rt.set_reduced_motion(true);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("card").unwrap().rect, rt.find("card").unwrap());
}
#[test]
fn an_exiting_parent_does_not_keep_a_second_copy() {
    let mut rt = Runtime::new(false, |right| {
        let card = el::el().key("card").flight("hero").size(40.)
            .fill(Rgba::new(1.,0.,0.,1.));
        el::row().child(el::el().key(if *right { "right" } else { "left" })
            .pad(if *right { 80. } else { 0. })
            .transition(el::Transition::fade()).child(card))
    }, Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE), (300,200),1.).unwrap();
    rt.update(|s| *s = true); rt.frame(Duration::ZERO);
    assert_eq!(rt.list().ops.iter().filter(|op| matches!(op,stillkit::Op::Fill { .. })).count(), 1);
}
