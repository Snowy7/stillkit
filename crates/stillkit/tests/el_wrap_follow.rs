//! `flow_wrap` boxes lay tiles onto further lines as the width allows; `follow_end` scroll
//! views (and virtual lists) stay at their end while content grows, unless scrolled back.

use std::time::Duration;

use stillkit::Theme;
use stillkit::el::{El, Runtime, col, el, row, text, virtual_list};
use stillkit::theme::{Scheme, Style};

fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

fn tiles(n: &usize) -> El<usize> {
    col().child(
        row()
            .key("grid")
            .w(320.0)
            .gap(10.0)
            .flow_wrap()
            .children((0..*n).map(|i| el().key(format!("t{i}")).size(100.0))),
    )
}

#[test]
fn wrapped_tiles_fill_rows_then_start_new_ones() {
    let rt = Runtime::new(7, tiles, theme(), (400, 400), 1.0).unwrap();
    let at = |i: usize| rt.find(&format!("t{i}")).unwrap();
    // Three 100 px tiles and two 10 px gaps fit in 320 px; the fourth starts a new row.
    assert_eq!(at(0).top, at(2).top);
    assert_eq!(at(3).left, at(0).left);
    assert_eq!(at(3).top, at(0).bottom + 10.0);
    assert_eq!(rt.find("grid").unwrap().height(), 3.0 * 100.0 + 2.0 * 10.0);
}

fn log(n: &usize) -> El<usize> {
    col().child(
        virtual_list("log", *n, 20.0, |range| {
            range
                .map(|i| text(format!("line {i}")).key(format!("l{i}")).h(20.0))
                .collect()
        })
        .h(200.0)
        .follow_end(),
    )
}

#[test]
fn a_following_list_shows_what_arrives_until_scrolled_back() {
    let mut rt = Runtime::new(5, log, theme(), (300, 300), 1.0).unwrap();
    rt.frame(Duration::ZERO);
    assert_eq!(
        rt.scroll_offset("log"),
        Some(0.0),
        "content shorter than the view"
    );
    // Grows past the view: it follows to the end, and the last line is built.
    rt.update(|n| *n = 100);
    rt.frame(Duration::from_millis(16));
    assert_eq!(rt.scroll_offset("log"), Some(100.0 * 20.0 - 200.0));
    assert!(rt.find("l99").is_some(), "the newest line is built");
    // Scrolled back, it holds its place as more arrives.
    let list = rt.find("log").unwrap();
    rt.wheel_at(list.center().x, list.center().y, 120);
    for _ in 0..60 {
        rt.frame(Duration::from_millis(16));
    }
    let held = rt.scroll_offset("log").unwrap();
    assert!(held < 1800.0);
    rt.update(|n| *n = 150);
    rt.frame(Duration::from_millis(16));
    assert_eq!(rt.scroll_offset("log"), Some(held));
}
