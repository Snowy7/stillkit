//! Raw pointer and wheel events, headless: a press holds the pointer through moves outside
//! the element, moves within a frame merge, hover runs no view unless the handler says so,
//! double clicks count, and the wheel goes to the innermost `on_wheel` over scroll views.

use std::time::Duration;

use stillkit::el::{El, Modifiers, PointerPhase, Runtime, col, el};
use stillkit::{Scheme, Style, Theme};

const FRAME: Duration = Duration::from_millis(16);

#[derive(Default)]
struct S {
    log: Vec<(PointerPhase, f32, f32, f32)>,
    value: f32,
    zoom: f32,
    hovers: u32,
}

fn view(_: &S) -> El<S> {
    col().pad(10.0).gap(10.0)
        .child(el().key("scrub").w(100.0).h(20.0).on_pointer(|s: &mut S, e, _| {
            s.log.push((e.phase, e.delta.x, e.local.x, e.travel().x));
            if e.phase == PointerPhase::Move {
                s.value += e.delta.x * if e.mods.shift { 0.1 } else { 1.0 };
            }
            if e.phase == PointerPhase::Hover {
                s.hovers += 1;
                return false;
            }
            if e.phase == PointerPhase::Down && e.clicks == 2 {
                s.value = 0.0;
            }
            true
        }))
        .child(el().key("list").h(60.0).scroll(stillkit::el::Axis::Vertical).children((0..20).map(|i| {
            el().h(20.0).key(format!("r{i}")).child(el().key(if i == 0 { "canvas" } else { "" }).w(40.0).h(20.0).on_wheel(|s: &mut S, w, _| {
                s.zoom += w.delta;
                true
            }))
        })))
}

fn rt() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), view, Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE), (320, 240), 2.0).unwrap();
    rt.frame(Duration::ZERO);
    rt
}

#[test]
fn a_press_holds_the_pointer_and_moves_merge_per_frame() {
    let mut rt = rt();
    let r = rt.find("scrub").unwrap(); // device px at scale 2
    let (x, y) = (r.left + 20.0, r.center().y);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.frame(FRAME);
    // Three moves in one frame, the last far outside the element (and the window).
    rt.pointer_move(x + 20.0, y);
    rt.pointer_move(x + 40.0, y + 300.0);
    rt.set_modifiers(Modifiers::SHIFT);
    rt.pointer_move(x + 1000.0, y);
    rt.frame(FRAME);
    rt.pointer_up(x + 1000.0, y);
    rt.frame(FRAME);
    let s = rt.state();
    let phases: Vec<PointerPhase> = s.log.iter().map(|l| l.0).collect();
    assert_eq!(&phases[phases.len() - 3..], &[PointerPhase::Down, PointerPhase::Move, PointerPhase::Up]);
    let moved = s.log.iter().find(|l| l.0 == PointerPhase::Move).unwrap();
    assert_eq!(moved.1, 500.0, "one merged move: 1000 device px at scale 2");
    assert_eq!(moved.3, 500.0, "travel from the press");
    assert_eq!(s.value, 50.0, "the merged move carries the latest modifiers (Shift: fine)");
}

#[test]
fn hovering_runs_no_view_unless_the_handler_changed_something() {
    let mut rt = rt();
    let r = rt.find("scrub").unwrap();
    let runs = rt.view_runs();
    for dx in [10.0, 20.0, 30.0] {
        rt.pointer_move(r.left + dx, r.center().y);
        rt.frame(FRAME);
    }
    assert_eq!(rt.state().hovers, 3);
    assert_eq!(rt.view_runs(), runs, "hover returned false");
    let deltas: Vec<f32> = rt.state().log.iter().map(|l| l.1).collect();
    assert_eq!(deltas, [0.0, 5.0, 5.0], "hover deltas from the last hover, design px");
}

#[test]
fn a_double_click_counts_two() {
    let mut rt = rt();
    let r = rt.find("scrub").unwrap();
    rt.update(|s| s.value = 7.0);
    rt.pointer_move(r.center().x, r.center().y);
    rt.handle(&stillkit::Event::DoubleClick(stillkit::Pointer { at: r.center(), screen: (0, 0) }, stillkit::Button::Left));
    rt.frame(FRAME);
    assert_eq!(rt.state().value, 0.0);
}

#[test]
fn the_wheel_goes_to_the_element_not_the_scroll_view_around_it() {
    let mut rt = rt();
    let c = rt.find("canvas").unwrap();
    rt.pointer_move(c.center().x, c.center().y);
    rt.wheel_at(c.center().x, c.center().y, 240);
    rt.frame(FRAME);
    assert_eq!(rt.state().zoom, 2.0);
    assert_eq!(rt.scroll_offset("list"), Some(0.0), "the list stayed put");
    // Outside the canvas the list scrolls as before.
    let list = rt.find("list").unwrap();
    rt.pointer_move(list.right - 4.0, list.center().y);
    rt.wheel_at(list.right - 4.0, list.center().y, -120);
    for _ in 0..60 {
        rt.frame(FRAME);
    }
    assert!(rt.scroll_offset("list").unwrap() > 0.0);
}
