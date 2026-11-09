//! Native layers, headless: a `native` element clears its box to transparent (a hole the
//! host's layer shows through), content after it paints over the hole, and
//! `Runtime::natives` reports the visible box, cut by scroll views, or nothing once gone.

use std::time::Duration;

use stillkit::el::{El, Runtime, col, el, native};
use stillkit::{Op, Rect, Rgba, Scheme, Style, Theme};

#[derive(Default)]
struct S {
    shown: bool,
}

fn view(s: &S) -> El<S> {
    let list = el().key("list").h(100.0).scroll(stillkit::el::Axis::Vertical).child(el().h(60.0)).child(native("scrolled").h(120.0));
    let mut root = col().pad(10.0).gap(10.0).fill(Rgba::new(0.1, 0.1, 0.1, 1.0));
    if s.shown {
        root = root.child(native("viewport").key("vp").h(80.0).child(el().key("pill").w(40.0).h(20.0).fill(Rgba::new(1.0, 0.0, 0.0, 1.0))));
    }
    root.child(list)
}

fn rt() -> Runtime<S> {
    let theme = Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
    let mut rt = Runtime::new(S { shown: true }, view, theme, (300, 400), 1.0).unwrap();
    rt.frame(Duration::ZERO);
    rt
}

#[test]
fn a_native_element_is_a_hole_under_what_follows() {
    let rt = rt();
    let vp = rt.find("vp").unwrap();
    let ops = &rt.list().ops;
    let hole = ops.iter().position(|op| matches!(op, Op::Hole { rect } if *rect == vp)).expect("a hole over its box");
    let pill = ops.iter().position(|op| matches!(op, Op::Fill { color, .. } if color.r == 1.0)).expect("its child paints");
    assert!(pill > hole, "children paint over the hole");
    let natives = rt.natives();
    assert_eq!(natives[0], ("viewport".into(), vp));
}

#[test]
fn a_scrolled_layer_is_cut_to_its_view_and_a_removed_one_is_unlisted() {
    let mut rt = rt();
    let list = rt.find("list").unwrap();
    let (_, scrolled) = rt.natives().into_iter().find(|(n, _)| &**n == "scrolled").unwrap();
    assert_eq!(scrolled, Rect::new(list.left, list.top + 60.0, list.width(), 40.0), "only the part inside the view");
    rt.update(|s| s.shown = false);
    rt.frame(Duration::from_millis(16));
    assert!(rt.natives().iter().all(|(n, _)| &**n != "viewport"), "gone: the host hides its layer");
}
