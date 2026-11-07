//! Motion snapshots use the same virtual-clock Script/--at path as el::run.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "../examples/support/mod.rs"]
mod support;
use std::{path::Path, time::Duration};
use stillkit::{
    Image,
    el::{Runtime, Script},
};

fn runtime(component: usize) -> Runtime<support::State> {
    let theme = support::themes().remove(0).1;
    let bg = theme.color.surface.with_alpha(1.);
    let mut rt = Runtime::new(
        support::State::default(),
        move |s| {
            stillkit::el::col()
                .pad(12.)
                .child(support::component(component, s, false))
        },
        theme,
        (280, 100),
        1.,
    )
    .unwrap();
    rt.set_background(Some(bg));
    rt.frame(Duration::ZERO);
    rt
}
fn settle(rt: &mut Runtime<support::State>) {
    for _ in 0..200 {
        if !rt.animating() {
            return;
        }
        rt.frame(Duration::from_millis(16));
    }
    panic!("motion did not settle");
}
#[test]
fn selection_moves_without_rebuilding_and_stops() {
    for (component, click, part) in [
        (2, "target", "target/thumb"),
        (7, "target/option-2", "target/selection"),
    ] {
        let mut rt = runtime(component);
        assert!(!rt.animating());
        let start = rt.presented(part).unwrap().rect;
        assert!(rt.click(click));
        rt.frame(Duration::ZERO);
        let end = rt.find(part).unwrap();
        let root = rt.find("target").unwrap();
        assert!(
            end.right <= root.right && end.bottom <= root.bottom,
            "pill {end:?} outside {root:?}"
        );
        assert_ne!(start, end);
        rt.frame(Duration::from_millis(50));
        let mid = rt.presented(part).unwrap().rect;
        assert!(
            mid.left > start.left && mid.left < end.left,
            "{mid:?}, {start:?}, {end:?}"
        );
        let runs = rt.view_runs();
        settle(&mut rt);
        assert_eq!(rt.view_runs(), runs);
        assert_eq!(rt.presented(part).unwrap().rect, end);
        assert!(!rt.frame(Duration::from_secs(1)));
    }
}
#[test]
fn reduced_travel_instant_updates_and_drag_land_at_once() {
    let mut rt = runtime(2);
    rt.set_reduced_motion(true);
    rt.click("target");
    rt.frame(Duration::ZERO);
    assert_eq!(
        rt.presented("target/thumb").unwrap().rect,
        rt.find("target/thumb").unwrap()
    );
    settle(&mut rt);
    rt.update_instant(|s| s.on = false);
    rt.frame(Duration::ZERO);
    assert!(!rt.animating());
    let mut rt = runtime(5);
    rt.update(|s| s.value = 0.8);
    rt.frame(Duration::ZERO);
    assert!(rt.animating());
    settle(&mut rt);
    let r = rt.find("target").unwrap();
    rt.pointer_down(r.left + r.width() * 0.2, r.center().y);
    rt.frame(Duration::ZERO);
    assert_eq!(
        rt.presented("target/fill").unwrap().rect,
        rt.find("target/fill").unwrap()
    );
}
fn strip(frames: &[Image]) -> Image {
    let (w, h) = (frames[0].width, frames[0].height);
    let width = w * frames.len() as u32;
    let mut pixels = vec![0; (width * h * 4) as usize];
    for (n, f) in frames.iter().enumerate() {
        for y in 0..h as usize {
            let at = (y * width as usize + n * w as usize) * 4;
            pixels[at..at + w as usize * 4]
                .copy_from_slice(&f.pixels[y * w as usize * 4..(y + 1) * w as usize * 4]);
        }
    }
    Image {
        width,
        height: h,
        pixels,
    }
}
#[test]
fn golden_motion_strips() {
    for (name, component, click) in [
        ("switch", 2, "target"),
        ("segmented", 7, "target/option-2"),
        ("checkbox", 3, "target"),
    ] {
        let script = Script::parse(&format!(r#"[{{"at":0.1,"click":"{click}"}}]"#)).unwrap();
        let frames: Vec<_> = [0.1, 0.15, 0.2, 0.3, 0.45, 1.5]
            .into_iter()
            .map(|at| {
                let mut rt = runtime(component);
                rt.play(&script, Some(Duration::from_secs_f64(at)));
                rt.frame(Duration::ZERO);
                rt.snap().unwrap()
            })
            .collect();
        stillkit::golden::check(
            &strip(&frames),
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../docs/media/kit/motion-{name}.png")),
        )
        .unwrap();
    }
}
