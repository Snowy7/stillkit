//! Gradient motion and native pixels without a window or GPU device.
#[allow(dead_code)]
#[path = "../examples/el_effects.rs"]
mod effects;
use std::time::Duration;
use stillkit::el::Runtime;
use stillkit::{Op, Scheme, Theme};
fn runtime() -> Runtime<bool> {
    Runtime::new(
        false,
        effects::view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (320, 340),
        1.0,
    )
    .unwrap()
}
fn gradient(rt: &Runtime<bool>) -> stillkit::paint::Gradient {
    rt.list()
        .ops
        .iter()
        .find_map(|op| {
            if let Op::Gradient { gradient, .. } = op {
                Some(gradient.clone())
            } else {
                None
            }
        })
        .unwrap()
}
#[test]
fn stops_and_angle_spring_without_running_the_view_and_settle_to_idle() {
    let mut rt = runtime();
    let before = gradient(&rt);
    rt.update(|on| *on = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(16));
    let middle = gradient(&rt);
    assert_ne!(before, middle);
    assert_ne!(before.stops[0].color, middle.stops[0].color);
    assert!(middle.stops[0].offset > 0.0 && middle.stops[0].offset < 0.2);
    assert!(middle.angle.unwrap() > 0.0 && middle.angle.unwrap() < 1.4);
    let views = rt.view_runs();
    for _ in 0..300 {
        rt.frame(Duration::from_millis(16));
    }
    assert!(!rt.animating());
    assert_eq!(rt.view_runs(), views);
    assert_eq!(gradient(&rt).stops.as_ref(), effects::COOL);
    let paints = rt.paints();
    assert!(!rt.frame(Duration::from_secs(1)));
    assert_eq!(rt.paints(), paints);
}
#[test]
fn gradient_damage_is_local_and_identical_frames_cost_no_redraw() {
    let mut rt = runtime();
    let before = rt.list().clone();
    assert!(before.damage(&before).is_none());
    rt.update(|on| *on = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(16));
    let damage = rt.list().damage(&before).unwrap();
    assert!(damage.left >= 19.0 && damage.right <= 301.0);
}
#[cfg(feature = "golden")]
#[test]
fn native_gradient_pixels() {
    let rt = runtime();
    let mut ui = stillkit::Ui::new().unwrap();
    let image = stillkit::render(&mut ui, rt.list(), 320, 340).unwrap();
    // Linear fill has different ends, its rounded corner stays on the background.
    assert_ne!(image.pixel(30, 90), image.pixel(280, 90));
    assert_ne!(image.pixel(160, 170), image.pixel(30, 170));
    assert_eq!(image.pixel(20, 20), image.pixel(0, 0));
    #[cfg(windows)]
    stillkit::golden::check(
        &image,
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden/el-effects-gradients.png"),
    )
    .unwrap();
}

#[test]
fn retargeting_a_gradient_preserves_the_presented_stops() {
    let mut rt = runtime();
    rt.update(|on| *on = true);
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(80));
    let before = gradient(&rt);
    rt.update(|on| *on = false);
    rt.frame(Duration::ZERO);
    let after = gradient(&rt);
    for (a, b) in before.stops.iter().zip(after.stops.iter()) {
        assert!((a.color.r - b.color.r).abs() < 0.01);
    }
    assert!((before.angle.unwrap() - after.angle.unwrap()).abs() < 0.001);
}
#[test]
fn empty_and_single_stop_gradients_are_safe_for_native_brushes() {
    use stillkit::{
        Rgba,
        paint::{Gradient, GradientStop},
    };
    let g = Gradient::new(&[], Some(f32::NAN));
    assert_eq!(g.stops.len(), 2);
    assert_eq!(g.stops[0].color, Rgba::TRANSPARENT);
    assert_eq!(g.angle, Some(0.0));
    let g = Gradient::new(
        &[GradientStop {
            offset: 4.0,
            color: Rgba::HIT,
        }],
        None,
    );
    assert!(
        g.stops
            .iter()
            .all(|s| s.color == Rgba::HIT && s.offset <= 1.0)
    );
}

#[test]
fn reduced_motion_lands_gradient_animation_within_the_short_fade() {
    let mut rt = runtime();
    rt.set_reduced_motion(true);
    rt.frame(Duration::ZERO);
    rt.update(|on| *on = true);
    rt.frame(Duration::ZERO);
    for _ in 0..60 {
        rt.frame(Duration::from_millis(16));
    }
    assert!(!rt.animating());
    assert_eq!(gradient(&rt).stops.as_ref(), effects::COOL);
}

#[test]
fn gradient_border_matches_the_native_stroke_after_an_icon() {
    use stillkit::{DisplayList, Rect, Rgba};
    let list = DisplayList {
        ops: vec![
            Op::Icon { rect: Rect::new(0.0, 0.0, 6.0, 6.0), icon: stillkit::icons::Icon::Check, weight: 1.0, color: Rgba::new(1.0, 1.0, 1.0, 1.0) },
            Op::Gradient { rect: Rect::new(12.0, 12.0, 20.0, 20.0), radius: 0.0, width: 8.0, gradient: stillkit::paint::Gradient::new(effects::WARM, Some(0.0)) },
        ],
        ..Default::default()
    };
    let mut ui = stillkit::Ui::new().unwrap();
    let image = stillkit::render(&mut ui, &list, 44, 44).unwrap();
    let mut reference = list.clone();
    reference.ops[1] = Op::Stroke { rect: Rect::new(12.0, 12.0, 20.0, 20.0), radius: 0.0, width: 8.0, color: Rgba::new(1.0, 1.0, 1.0, 1.0) };
    let reference = stillkit::render(&mut ui, &reference, 44, 44).unwrap();
    for (actual, expected) in image.pixels.as_chunks::<4>().0.iter().zip(reference.pixels.as_chunks::<4>().0) {
        assert!(actual[3].abs_diff(expected[3]) <= 8, "gradient borders must keep the native stroke's shape after an icon");
    }
    assert_eq!(image.pixel(20, 20)[3], 0, "the border leaves its interior clear");
}
