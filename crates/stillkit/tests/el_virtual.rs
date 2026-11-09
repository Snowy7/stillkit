//! Virtual lists, headless: 100,000 rows build only those near sight, small scrolls run no
//! view, a scroll past what was built builds again in the same frame, rows land where their
//! index says, and the scroll extent is the whole list.

use std::time::Duration;

use stillkit::el::{El, Runtime, col, el, text, virtual_list};
use stillkit::{Scheme, Style, Theme};

const ROWS: usize = 100_000;
const ROW: f32 = 20.0;

fn view(_: &()) -> El<()> {
    col().child(virtual_list("list", ROWS, ROW, |range| range.map(|i| el().key(format!("row{i}")).h(ROW).child(text(format!("Row {i}")))).collect()).h(400.0))
}

fn rt() -> Runtime<()> {
    let theme = Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
    let mut rt = Runtime::new((), view, theme, (300, 400), 1.0).unwrap();
    rt.set_reduced_motion(true); // wheel notches land at once
    rt.frame(Duration::ZERO);
    rt.frame(Duration::from_millis(16));
    rt
}

fn built(rt: &Runtime<()>) -> usize {
    rt.tree().iter().filter(|n| n.key.as_deref().is_some_and(|k| k.starts_with("row"))).count()
}

#[test]
fn only_rows_near_sight_are_built() {
    let rt = rt();
    let n = built(&rt);
    assert!(n > 20 && n < 200, "a viewport and some: {n}");
    assert!(rt.find("row0").is_some() && rt.find("row5000").is_none());
}

#[test]
fn small_scrolls_run_no_view_and_long_ones_build_in_the_same_frame() {
    let mut rt = rt();
    let runs = rt.view_runs();
    rt.pointer_move(150.0, 200.0);
    rt.wheel_at(150.0, 200.0, -120); // one notch
    rt.frame(Duration::from_millis(16));
    assert_eq!(rt.view_runs(), runs, "still within what was built");
    // Far down: wheel until the offset passes the built rows.
    for _ in 0..80 {
        rt.wheel_at(150.0, 200.0, -1200);
        rt.frame(Duration::from_millis(16));
    }
    let offset = rt.scroll_offset("list").unwrap();
    assert!(offset > 2000.0, "{offset}");
    let first = (offset / ROW) as usize;
    let row = rt.find(&format!("row{}", first + 2)).expect("the rows in sight are built");
    let list = rt.find("list").unwrap();
    // Laid out (before the scroll offset moves it) at its index's place in the list.
    let expect = list.top + (first + 2) as f32 * ROW;
    assert!((row.top - expect).abs() < 1.0, "row at {} not {}", row.top, expect);
    assert!(rt.view_runs() > runs, "the view ran again");
}

#[test]
fn the_extent_is_the_whole_list() {
    let mut rt = rt();
    rt.pointer_move(150.0, 200.0);
    for _ in 0..3000 {
        rt.wheel_at(150.0, 200.0, -12000);
        rt.frame(Duration::from_millis(16));
    }
    let offset = rt.scroll_offset("list").unwrap();
    assert_eq!(offset, ROWS as f32 * ROW - 400.0, "scrolled to the very end");
    assert!(rt.find(&format!("row{}", ROWS - 1)).is_some(), "the last row is built");
}
