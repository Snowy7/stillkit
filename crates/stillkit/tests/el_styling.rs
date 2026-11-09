//! Styling for dense tools, headless: hover and pressed looks (retargeted without running the
//! view), borders, typography overrides and alignment, and per-element pointer shapes.

use std::time::Duration;

use stillkit::el::{El, Runtime, TextAlign, col, el, row, text, text_input};
use stillkit::{Cursor, Op, Rect, Rgba, Scheme, Style, Theme};

const FRAME: Duration = Duration::from_millis(16);
const RED: Rgba = Rgba::new(1.0, 0.0, 0.0, 1.0);
const BLUE: Rgba = Rgba::new(0.0, 0.0, 1.0, 1.0);
const GREEN: Rgba = Rgba::new(0.0, 1.0, 0.0, 1.0);

fn theme() -> Theme {
    Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

fn runtime(view: fn(&()) -> El<()>) -> Runtime<()> {
    let mut rt = Runtime::new((), view, theme(), (320, 240), 2.0).unwrap();
    rt.frame(Duration::ZERO);
    rt
}

fn settle(rt: &mut Runtime<()>) {
    for _ in 0..200 {
        if !rt.frame(FRAME) && !rt.animating() {
            return;
        }
    }
    panic!("it settles");
}

/// The fill drawn exactly over `rect` with this colour, if any.
fn filled(rt: &Runtime<()>, rect: Rect, color: Rgba) -> bool {
    rt.list().ops.iter().any(|op| matches!(op, Op::Fill { rect: r, color: c, .. } if *r == rect && *c == color))
}

fn states(_: &()) -> El<()> {
    col().pad(10.0).child(
        el().key("row")
            .w(120.0)
            .h(24.0)
            .fill(BLUE)
            .on_press(|_| {})
            .motion(stillkit::Spring::apple(0.1, 0.0))
            .hovered(|e| e.fill(RED))
            .pressed(|e| e.fill(GREEN)),
    )
}

#[test]
fn hover_and_press_looks_spring_in_without_running_the_view() {
    let mut rt = runtime(states);
    let r = rt.find("row").unwrap();
    assert!(filled(&rt, r, BLUE));
    rt.pointer_move(r.center().x, r.center().y);
    rt.frame(FRAME);
    assert!(rt.animating(), "the hover look springs in");
    settle(&mut rt);
    assert!(filled(&rt, r, RED), "hovered: {:?}", rt.list().ops);
    rt.pointer_down(r.center().x, r.center().y);
    settle(&mut rt);
    assert!(filled(&rt, r, GREEN), "pressed lies over hover");
    rt.pointer_up(r.center().x, r.center().y);
    settle(&mut rt);
    assert!(filled(&rt, r, RED), "released, still hovered");
    rt.pointer_move(300.0, 230.0);
    settle(&mut rt);
    assert!(filled(&rt, r, BLUE), "left: its own look again");
    assert_eq!(rt.view_runs(), 2, "only the press's handler ran the view");
}

fn bordered(_: &()) -> El<()> {
    col().pad(10.0).child(el().key("box").w(100.0).h(40.0).radius(6.0).border(1.0, RED))
}

#[test]
fn a_border_strokes_inside_the_edge_at_device_width() {
    let rt = runtime(bordered);
    let r = rt.find("box").unwrap();
    let stroke = rt.list().ops.iter().find_map(|op| match op {
        Op::Stroke { rect, radius, width, color } if *color == RED => Some((*rect, *radius, *width)),
        _ => None,
    });
    let (rect, radius, width) = stroke.expect("a border is a stroke");
    assert_eq!(width, 2.0, "1 design px at scale 2");
    assert_eq!(rect, r.inflate(-1.0), "centred on a line one half-width inside the edge");
    assert_eq!(radius, 11.0, "follows the radius, inset");
}

fn typed(_: &()) -> El<()> {
    col().pad(10.0)
        .child(text("12.5").key("end").w(200.0).text_align(TextAlign::End))
        .child(text("mid").key("mid").w(200.0).text_align(TextAlign::Center))
        .child(text("mono").key("mono").family("Menlo").weight(600).text_size(20.0))
}

#[test]
fn text_takes_its_own_face_and_alignment() {
    let rt = runtime(typed);
    let text_op = |s: &str| {
        rt.list().ops.iter().find_map(|op| match op {
            Op::Text { rect, text, style, .. } if &**text == s => Some((*rect, style.clone())),
            _ => None,
        })
    };
    let end = rt.find("end").unwrap();
    let (rect, _) = text_op("12.5").unwrap();
    assert!((rect.right - end.right).abs() <= 1.0, "end-aligned: {rect:?} in {end:?}");
    let mid = rt.find("mid").unwrap();
    let (rect, _) = text_op("mid").unwrap();
    assert!((rect.center().x - mid.center().x).abs() <= 1.0, "centred: {rect:?} in {mid:?}");
    let (_, style) = text_op("mono").unwrap();
    assert_eq!((&*style.family, style.weight, style.size), ("Menlo", 600, 40.0));
    let mono = rt.find("mono").unwrap();
    assert!(mono.height() >= 40.0, "the line grows with the size: {mono:?}");
}

fn shaped(_: &()) -> El<()> {
    row().pad(10.0).gap(10.0)
        .child(el().key("split").w(8.0).h(100.0).on_drag(|_, _, _| {}).cursor(Cursor::ResizeHorizontal))
        .child(text_input("name").w(120.0))
        .child(el().key("plain").w(60.0).h(40.0).on_press(|_| {}))
}

#[test]
fn the_pointer_takes_the_innermost_shape_and_keeps_it_through_a_press() {
    let mut rt = runtime(shaped);
    let split = rt.find("split").unwrap();
    rt.pointer_move(split.center().x, split.center().y);
    assert_eq!(rt.cursor(), Cursor::ResizeHorizontal);
    let name = rt.find("name").unwrap();
    rt.pointer_move(name.center().x, name.center().y);
    assert_eq!(rt.cursor(), Cursor::Text, "fields show an I-beam");
    let plain = rt.find("plain").unwrap();
    rt.pointer_move(plain.center().x, plain.center().y);
    assert_eq!(rt.cursor(), Cursor::Arrow);
    // A splitter drag keeps its shape when the pointer outruns it.
    rt.pointer_move(split.center().x, split.center().y);
    rt.pointer_down(split.center().x, split.center().y);
    rt.pointer_move(plain.center().x, plain.center().y);
    assert_eq!(rt.cursor(), Cursor::ResizeHorizontal);
    rt.pointer_up(plain.center().x, plain.center().y);
    assert_eq!(rt.cursor(), Cursor::Arrow);
}
