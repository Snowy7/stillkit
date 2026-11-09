//! Files dropped from other apps, headless: the innermost `on_drop_files` element under the
//! drag takes its `dropping` look (without running the view) and the dropped paths.

use std::path::PathBuf;
use std::time::Duration;

use stillkit::el::{El, Runtime, col, el};
use stillkit::{Op, Point, Rgba, Scheme, Style, Theme};

const IDLE: Rgba = Rgba::new(0.1, 0.1, 0.1, 1.0);
const HOT: Rgba = Rgba::new(0.0, 0.5, 1.0, 1.0);

#[derive(Default)]
struct S {
    outer: Vec<PathBuf>,
    inner: Vec<PathBuf>,
}

fn view(_: &S) -> El<S> {
    col().key("outer").pad(40.0).fill(IDLE).on_drop_files(|s: &mut S, p| s.outer = p.to_vec()).dropping(|e| e.fill(HOT)).child(
        el().key("inner").w(100.0).h(60.0).fill(IDLE).on_drop_files(|s: &mut S, p| s.inner = p.to_vec()).dropping(|e| e.fill(HOT)),
    )
}

fn hot(rt: &Runtime<S>, key: &str) -> bool {
    let r = rt.find(key).unwrap();
    rt.list().ops.iter().any(|op| matches!(op, Op::Fill { rect, color, .. } if *rect == r && *color == HOT))
}

#[test]
fn the_innermost_target_lights_up_and_takes_the_files() {
    let mut rt = Runtime::new(S::default(), view, Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE), (300, 300), 1.0).unwrap();
    rt.frame(Duration::ZERO);
    let inner = rt.find("inner").unwrap().center();
    let runs = rt.view_runs();
    rt.files_over(Some(inner));
    rt.frame(Duration::from_millis(16));
    assert!(hot(&rt, "inner") && !hot(&rt, "outer"), "only the innermost target");
    assert_eq!(rt.view_runs(), runs, "hover feedback runs no view");
    rt.files_over(Some(Point::new(5.0, 5.0)));
    rt.frame(Duration::from_millis(16));
    assert!(hot(&rt, "outer") && !hot(&rt, "inner"));
    rt.drop_files(inner.x, inner.y, vec![PathBuf::from("/tmp/a.glb"), PathBuf::from("/tmp/b.png")]);
    rt.frame(Duration::from_millis(16));
    assert_eq!(rt.state().inner.len(), 2);
    assert!(rt.state().outer.is_empty());
    assert!(!hot(&rt, "outer") && !hot(&rt, "inner"), "the look goes with the drop");
}
