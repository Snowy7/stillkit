//! Scroll views, headless, on the inbox example: content and hits clipped to the viewport,
//! the header untouched, wheel and keys scrolling without running the view, damage inside the
//! viewport, the ticker disarming once the scrollbar fades, and focus and `scroll_into_view`
//! revealing hidden rows.

#[allow(dead_code)]
#[path = "../examples/el_scroll.rs"]
mod inbox;

use std::time::Duration;

use inbox::{Inbox, theme, view};
use stillkit::el::{KeyCode, Runtime};
use stillkit::{Op, Rect};

const FRAME: Duration = Duration::from_millis(16);

fn runtime() -> Runtime<Inbox> {
    Runtime::new(Inbox { rows: 60, picked: None }, view, theme(), (320, 400), 1.0).unwrap()
}

/// Frames of `dt` until nothing moves; how long that took.
fn settle<S>(rt: &mut Runtime<S>) -> Duration {
    let start = rt.now();
    while rt.animating() {
        rt.frame(FRAME);
        assert!(rt.now() - start < Duration::from_secs(5), "never settled");
    }
    rt.now() - start
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.left >= outer.left && inner.top >= outer.top && inner.right <= outer.right && inner.bottom <= outer.bottom
}

/// Whether the row keyed `key` shows wholly inside the list now.
fn shows(rt: &Runtime<Inbox>, key: &str) -> bool {
    let list = rt.presented("list").unwrap().rect;
    inside(rt.presented(key).unwrap().rect, list)
}

#[test]
fn content_and_hits_are_clipped_to_the_viewport_and_the_header_stays() {
    let rt = runtime();
    let list = rt.find("list").unwrap();
    let header = rt.find("header").unwrap();
    assert!(header.bottom <= list.top, "the header sits above the list");
    let clipped = rt.list().ops.iter().find_map(|op| match op {
        Op::Clipped { rect, ops } => Some((*rect, ops.len())),
        _ => None,
    });
    let (rect, ops) = clipped.expect("the rows are drawn in one clipped group");
    assert_eq!(rect, list);
    assert!(ops > 60, "every row is in it");
    // The last row is laid out far below and takes no hit.
    let last = rt.find("row-59").unwrap();
    assert!(last.top > list.bottom);
    let hit = |rt: &Runtime<Inbox>, at: stillkit::Point| rt.list().hit_at(at).map(|h| h.label.clone());
    assert_eq!(hit(&rt, last.center()), None, "below the viewport nothing is hit");
    // A row cut by the bottom edge is hit only inside it.
    let cut = (0..60)
        .map(|i| format!("row-{i}"))
        .find(|k| {
            let r = rt.find(k).unwrap();
            r.top < list.bottom && r.bottom > list.bottom
        })
        .expect("a row straddles the bottom edge");
    let hits = &rt.list().hits;
    let label = rt.presented(&cut).unwrap().label.unwrap();
    let h = hits.iter().find(|h| h.label == label).unwrap();
    assert_eq!(h.rect.bottom, list.bottom, "its hit is cut to the viewport");
    assert!(hits.iter().all(|h| h.rect.bottom <= list.bottom || h.rect.top < list.top));
}

#[test]
fn the_wheel_scrolls_without_running_the_view_and_damages_only_the_viewport() {
    let mut rt = runtime();
    let list = rt.find("list").unwrap();
    let views = rt.view_runs();
    let at = list.center();
    rt.wheel_at(at.x, at.y, -120);
    assert!(rt.animating(), "a notch glides");
    let mut frames = 0;
    while rt.animating() {
        if rt.frame(FRAME)
            && let Some(damage) = rt.damage()
        {
            assert!(inside(damage, list), "{damage:?} stays in {list:?}");
            frames += 1;
        }
        assert!(frames < 400, "never settled");
    }
    assert!(frames > 2, "it glided over several frames");
    assert_eq!(rt.view_runs(), views, "scrolling never runs the view");
    let s = rt.scale();
    assert_eq!(rt.scroll_offset("list"), Some((48.0 * s).round()));
    assert_eq!(rt.presented("list").unwrap().scroll, Some(48.0));
    // Rows moved up by the offset; the header did not.
    assert_eq!(rt.presented("row-0").unwrap().rect, rt.find("row-0").unwrap().offset(0.0, -48.0));
    assert_eq!(rt.presented("header").unwrap().rect, rt.find("header").unwrap());
    // A touchpad's small delta lands at once.
    rt.wheel_at(at.x, at.y, -30);
    rt.frame(FRAME);
    assert_eq!(rt.presented("list").unwrap().scroll, Some(60.0));
    // Wheeling outside the list scrolls nothing.
    rt.wheel_at(10.0, 10.0, -120);
    assert_eq!(rt.scroll_offset("list"), Some(60.0));
}

#[test]
fn the_ticker_disarms_once_the_scrollbar_fades() {
    let mut rt = runtime();
    let at = rt.find("list").unwrap().center();
    rt.wheel_at(at.x, at.y, -120);
    let took = settle(&mut rt);
    let motion = rt.theme().motion;
    let fade = Duration::from_secs_f32(motion.scrollbar_idle + motion.scrollbar_fade);
    assert!(took >= fade, "the bar shows, then fades ({took:?})");
    assert!(took < fade + Duration::from_millis(200), "then everything rests ({took:?})");
    // The last frame drew the bar clear.
    let thumb = rt.list().ops.iter().rev().find_map(|op| match op {
        Op::Fill { rect, color, .. } if rect.width() < 10.0 => Some(*color),
        _ => None,
    });
    assert_eq!(thumb.map(|c| c.a), Some(0.0));
    let paints = rt.paints();
    for _ in 0..20 {
        assert!(!rt.frame(FRAME));
    }
    assert_eq!(rt.paints(), paints, "nothing paints at rest");
}

#[test]
fn page_keys_and_arrows_scroll_a_focused_viewport() {
    let mut rt = runtime();
    let list = rt.find("list").unwrap();
    // A click on the list's padding focuses the list itself.
    rt.pointer_down(list.left + 1.0, list.top + 1.0);
    rt.pointer_up(list.left + 1.0, list.top + 1.0);
    rt.frame(FRAME);
    assert!(rt.presented("list").unwrap().focused);
    rt.key(KeyCode::PAGE_DOWN.0);
    let page = (list.height() * 0.85).round();
    assert_eq!(rt.scroll_offset("list"), Some(page));
    rt.key(KeyCode::DOWN.0);
    assert_eq!(rt.scroll_offset("list"), Some(page + 40.0));
    rt.key(KeyCode::END.0);
    let max = rt.scroll_offset("list").unwrap();
    rt.key(KeyCode::SPACE.0);
    assert_eq!(rt.scroll_offset("list"), Some(max), "clamped at the end");
    rt.key(KeyCode::HOME.0);
    assert_eq!(rt.scroll_offset("list"), Some(0.0));
    settle(&mut rt);
    assert_eq!(rt.view_runs(), 1);
}

#[test]
fn tab_onto_a_hidden_row_scrolls_it_into_view() {
    let mut rt = runtime();
    rt.tab(false);
    rt.frame(FRAME);
    assert!(rt.presented("jump").unwrap().focused, "the header's button first");
    // The list is no stop of its own: its rows are.
    rt.tab(false);
    rt.frame(FRAME);
    assert!(rt.presented("row-0").unwrap().focused);
    let hidden = (0..60).find(|i| !shows(&rt, &format!("row-{i}"))).unwrap();
    for _ in 0..hidden {
        rt.tab(false);
        rt.frame(FRAME);
    }
    let key = format!("row-{hidden}");
    assert!(rt.presented(&key).unwrap().focused);
    assert!(rt.list().hits.iter().any(|h| h.label == rt.presented(&key).unwrap().label.unwrap()));
    settle(&mut rt);
    assert!(shows(&rt, &key), "the focused row scrolled into view");
    assert!(rt.scroll_offset("list").unwrap() > 0.0);
    // Enter presses it, though it was hidden when focus arrived.
    rt.key(KeyCode::ENTER.0);
    rt.frame(FRAME);
    assert_eq!(rt.state().picked, Some(hidden));
    // Shift+Tab back to the first row scrolls back up.
    for _ in 0..hidden {
        rt.tab(true);
    }
    settle(&mut rt);
    assert!(rt.presented("row-0").unwrap().focused && shows(&rt, "row-0"));
    assert_eq!(rt.scroll_offset("list"), Some(0.0));
}

#[test]
fn a_handler_scrolls_a_row_into_view_once_the_view_ran() {
    let mut rt = runtime();
    assert!(rt.click("jump"));
    rt.frame(FRAME);
    assert_eq!(rt.state().picked, Some(59));
    settle(&mut rt);
    assert!(shows(&rt, "row-59"), "the last row shows");
    let list = rt.presented("list").unwrap().rect;
    let last = rt.presented("row-59").unwrap().rect;
    assert!(list.bottom - last.bottom <= 4.0, "just enough: it sits at the bottom");
    // The runtime's own call does the same at the next frame.
    rt.scroll_into_view("row-0");
    rt.frame(FRAME);
    settle(&mut rt);
    assert!(shows(&rt, "row-0"));
    // Fewer rows pull a scrolled list back within its content.
    rt.scroll_into_view("row-59");
    rt.frame(FRAME);
    settle(&mut rt);
    rt.update(|s| s.rows = 12);
    rt.frame(FRAME);
    let max = rt.scroll_offset("list").unwrap();
    let list = rt.find("list").unwrap();
    let bottom = rt.find("row-11").unwrap().bottom;
    assert!(bottom - max <= list.bottom + 4.0, "{bottom} - {max} in {list:?}");
}
