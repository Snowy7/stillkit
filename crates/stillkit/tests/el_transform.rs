//! Layout-independent affine paint, motion and exact input regions.
use std::{sync::Arc, time::Duration};
use stillkit::el::{self, El, Runtime};
use stillkit::{Image, Op, Point, Rect, Rgba, Scheme, Theme, Transform};
fn theme() -> Theme {
    Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}
fn view(on: &bool) -> El<bool> {
    el::el().pad(60.).child(
        el::el()
            .key("card")
            .w(100.)
            .h(60.)
            .fill(Rgba::new(0.8, 0.1, 0.2, 1.))
            .translate(if *on { 35. } else { 0. }, 10.)
            .scale(if *on { 1.3 } else { 1. })
            .rotate(if *on { 0.63 } else { 0. })
            .origin(0., 0.)
            .animate()
            .on_press(|s: &mut bool| *s = !*s)
            .child(el::text("Affine text"))
            .child(
                el::image(Arc::new(Image {
                    width: 2,
                    height: 2,
                    pixels: vec![
                        0, 255, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255,
                    ],
                }))
                .size(24.),
            ),
    )
}
fn runtime() -> Runtime<bool> {
    Runtime::new(false, view, theme(), (280, 240), 1.).unwrap()
}
#[test]
fn transforms_leave_layout_upright_and_hit_only_the_painted_box() {
    let mut rt = runtime();
    let before = rt.find("card").unwrap();
    rt.update_instant(|s| *s = true);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.find("card"), Some(before));
    let hit = rt.list().hits.iter().find(|h| h.actionable).unwrap();
    let xf = Transform::new(
        Point::new(before.left, before.top),
        0.63,
        (1.3, 1.3),
        (35., 10.),
    );
    assert!(hit.contains(xf.point(before.center())));
    assert!(!hit.contains(Point::new(hit.rect.left + 0.1, hit.rect.top + 0.1)));
    let original = hit.unmap(xf.point(before.center()));
    assert!((original.x - before.center().x).abs() < 0.001);
}
#[test]
fn interrupted_motion_keeps_pose_and_reduced_motion_snaps_to_idle() {
    let mut rt = runtime();
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(80));
    let before = rt.list().clone();
    let views = rt.view_runs();
    assert!(rt.animating());
    rt.update(|s| *s = false);
    rt.frame(Duration::ZERO);
    assert_eq!(before.ops, rt.list().ops);
    rt.set_reduced_motion(true);
    rt.update(|s| *s = true);
    rt.frame(Duration::ZERO);
    for _ in 0..100 {
        rt.frame(Duration::from_millis(16));
    }
    assert!(!rt.animating());
    assert_eq!(rt.view_runs(), views + 2);
    assert!(!rt.frame(Duration::from_secs(1)));
}
#[test]
fn nested_transforms_preserve_clipped_hits_and_collapsed_scales_take_no_hits() {
    let mut hit = stillkit::Hit::new(
        1,
        Rect::new(0., 0., 20., 10.),
        stillkit::Role::Button,
        "",
        true,
        None,
    );
    let first = Transform::new(Point::new(0., 0.), 0.7, (1., 1.), (20., 20.));
    hit.transform(first);
    hit.rect = hit.rect.intersect(Rect::new(20., 20., 8., 30.));
    let second = Transform::new(Point::new(0., 0.), -0.4, (-1., 2.), (100., 0.));
    hit.transform(second);
    assert!(!hit.contains(second.point(first.point(Point::new(19., 1.)))));
    hit.transform(Transform::new(Point::new(0., 0.), 0., (0., 0.), (0., 0.)));
    assert!(!hit.contains(Point::new(0., 0.)));
}
#[test]
fn op_translation_and_scaling_keep_affine_children_in_their_own_space() {
    let child = Op::Fill {
        rect: Rect::new(3., 4., 10., 8.),
        radius: 2.,
        color: Rgba::HIT,
    };
    let mut op = Op::Transformed {
        transform: Transform::IDENTITY,
        ops: vec![child.clone()].into(),
    };
    op.translate(7., 9.);
    op.scale(2.);
    let Op::Transformed { transform, ops } = op else {
        panic!()
    };
    assert_eq!(ops[0], child);
    assert_eq!(transform.point(Point::new(3., 4.)), Point::new(20., 26.));
}
#[test]
#[cfg(all(feature = "golden", windows))]
fn rotated_text_image_and_box_pixels() {
    let mut rt = runtime();
    rt.update_instant(|s| *s = true);
    rt.frame(Duration::ZERO);
    let image = rt.snap().unwrap();
    stillkit::golden::check(
        &image,
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/el-transforms.png"),
    )
    .unwrap();
}
