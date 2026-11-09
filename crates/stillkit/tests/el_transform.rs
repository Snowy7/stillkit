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

#[test]
#[cfg(windows)]
fn a_rotated_marker_clip_preserves_its_shape() {
    let mut ui = stillkit::Ui::new().unwrap();
    let list = stillkit::DisplayList {
        ops: vec![Op::Transformed {
            transform: Transform::new(
                Point::new(40., 40.),
                std::f32::consts::FRAC_PI_4,
                (1., 1.),
                (0., 0.),
            ),
            ops: vec![
                Op::Clip {
                    rect: Rect::new(30., 30., 20., 20.),
                },
                Op::Fill {
                    rect: Rect::new(0., 0., 80., 80.),
                    radius: 0.,
                    color: Rgba::HIT,
                },
                Op::Unclip,
            ]
            .into(),
        }],
        ..Default::default()
    };
    let image = stillkit::render(&mut ui, &list, 80, 80).unwrap();
    assert_eq!(image.pixel(27, 27)[3], 0);
    assert!(image.pixel(40, 40)[3] > 0);
}

#[test]
fn selection_uses_the_transformed_text_coordinates() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::el().pad(60.0).child(
                el::text("hello world")
                    .key("label")
                    .w(180.0)
                    .h(30.0)
                    .selectable()
                    .rotate(0.63)
                    .scale(1.2)
                    .translate(35.0, 20.0),
            )
        },
        theme(),
        (320, 240),
        1.0,
    )
    .unwrap();
    let rect = rt.find("label").unwrap();
    let xf = Transform::new(rect.center(), 0.63, (1.2, 1.2), (35.0, 20.0));
    let at = xf.point(Point::new(rect.left + 2.0, rect.top + 3.0));
    for _ in 0..2 {
        rt.pointer_down(at.x, at.y);
        rt.pointer_up(at.x, at.y);
    }
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("label").unwrap().selection, Some((0, 5)));
}

#[test]
fn keyed_click_follows_the_translated_control() {
    let mut rt = Runtime::new(
        0,
        |_| {
            el::el().pad(20.0).child(
                el::el()
                    .key("button")
                    .size(30.0)
                    .translate(150.0, 0.0)
                    .on_press(|n: &mut i32| *n += 1),
            )
        },
        theme(),
        (300, 180),
        1.0,
    )
    .unwrap();
    assert!(rt.click("button"));
    rt.frame(Duration::ZERO);
    assert_eq!(*rt.state(), 1);
}
