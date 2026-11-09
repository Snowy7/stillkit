//! Display selection stays outside application state and spans a document.
use std::{cell::RefCell, rc::Rc, time::Duration};
use stillkit::el::{self, Clipboard, KeyCode, Modifiers, Runtime};
use stillkit::{Scheme, Semantics, Theme};
#[derive(Clone, Default)]
struct Board(Rc<RefCell<String>>);
impl Clipboard for Board {
    fn get(&mut self) -> Option<String> {
        Some(self.0.borrow().clone())
    }
    fn set(&mut self, value: &str) {
        *self.0.borrow_mut() = value.into();
    }
}
fn document() -> (Runtime<()>, Board) {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::col()
                .pad(10.0)
                .gap(8.0)
                .selectable()
                .child(el::text("hello world").key("first").w(180.0))
                .child(el::text("second paragraph").key("second").w(180.0))
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (220, 140),
        1.0,
    )
    .unwrap();
    let board = Board::default();
    rt.set_clipboard(board.clone());
    rt.frame(Duration::ZERO);
    (rt, board)
}
#[test]
fn select_all_copies_document_order_and_publishes_ranges_without_running_view() {
    let (mut rt, board) = document();
    rt.click("first");
    let views = rt.view_runs();
    rt.key_with(KeyCode::of('A').0, Modifiers::CTRL);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    rt.frame(Duration::ZERO);
    assert_eq!(&*board.0.borrow(), "hello world\nsecond paragraph");
    assert_eq!(rt.presented("first").unwrap().selection, Some((0, 11)));
    assert_eq!(rt.presented("second").unwrap().selection, Some((0, 16)));
    assert_eq!(rt.view_runs(), views);
    assert!(
        rt.semantics()
            .iter()
            .any(|(_, s)| matches!(s, Semantics::Field(t) if t.caret.1 > 0))
    );
}
#[test]
fn drag_crosses_elements_and_reverse_drag_preserves_reading_order() {
    let (mut rt, board) = document();
    let a = rt.presented("first").unwrap().rect;
    let b = rt.presented("second").unwrap().rect;
    rt.pointer_down(a.left, a.top + 3.0);
    rt.pointer_move(b.right - 1.0, b.top + 3.0);
    rt.pointer_up(b.right - 1.0, b.top + 3.0);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello world\nsecond paragraph");
    rt.frame(Duration::from_millis(600));
    board.0.borrow_mut().clear();
    rt.pointer_down(b.right - 1.0, b.top + 3.0);
    rt.pointer_move(a.left, a.top + 3.0);
    rt.pointer_up(a.left, a.top + 3.0);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello world\nsecond paragraph");
}
#[test]
fn double_word_triple_paragraph_and_shift_extension() {
    let (mut rt, board) = document();
    let r = rt.presented("first").unwrap().rect;
    for _ in 0..2 {
        rt.pointer_down(r.left + 2.0, r.top + 3.0);
        rt.pointer_up(r.left + 2.0, r.top + 3.0);
    }
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello");
    rt.pointer_down(r.left + 2.0, r.top + 3.0);
    rt.pointer_up(r.left + 2.0, r.top + 3.0);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello world");
    rt.frame(Duration::from_millis(600));
    rt.pointer_down(r.left, r.top + 3.0);
    rt.pointer_up(r.left, r.top + 3.0);
    rt.key_with(KeyCode::END.0, Modifiers::SHIFT);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert!(!board.0.borrow().is_empty());
}
#[cfg(all(windows, feature = "golden"))]
#[test]
fn selected_document_golden() {
    let (mut rt, _) = document();
    rt.click("first");
    rt.key_with(KeyCode::of('A').0, Modifiers::CTRL);
    rt.frame(Duration::ZERO);
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/display-selection.png");
    stillkit::golden::check(&rt.snap().unwrap(), &path).unwrap();
}

#[test]
fn whitespace_unicode_and_separate_documents_keep_their_source() {
    let mut rt = Runtime::new((), |_| el::col()
        .child(el::text("  cafe\u{301}\n  \u{627}\u{644}\u{639}\u{631}\u{628}\u{64a}\u{629}  \u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{200d}\u{1f466}").key("unicode").w(100.0).selectable())
        .child(el::text("another document").key("other").selectable()),
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE), (240, 220), 1.0).unwrap();
    let board = Board::default();
    rt.set_clipboard(board.clone());
    rt.click("unicode");
    rt.key_with(KeyCode::of('A').0, Modifiers::CTRL);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    rt.frame(Duration::ZERO);
    assert_eq!(
        &*board.0.borrow(),
        "  cafe\u{301}\n  \u{627}\u{644}\u{639}\u{631}\u{628}\u{64a}\u{629}  \u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{200d}\u{1f466}"
    );
    assert_eq!(rt.presented("other").unwrap().selection, Some((0, 0)));
    rt.frame(Duration::from_secs(10)); // Let ordinary focus feedback settle.
    let paints = rt.paints();
    rt.frame(Duration::from_secs(10));
    assert_eq!(
        rt.paints(),
        paints,
        "display selection has no caret blink or idle work"
    );
}

#[test]
fn controls_inside_documents_keep_their_clicks() {
    let mut rt = Runtime::new(
        0u32,
        |_| {
            el::col()
                .selectable()
                .child(el::text("Document").key("body"))
                .child(
                    el::el()
                        .key("copy")
                        .on_press(|s: &mut u32| *s += 1)
                        .child(el::text("Copy").key("copy-label")),
                )
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (240, 140),
        1.0,
    )
    .unwrap();
    assert!(rt.click("copy"));
    rt.frame(Duration::ZERO);
    assert_eq!(*rt.state(), 1);
    assert_eq!(rt.presented("copy-label").unwrap().selection, None);
}

#[test]
fn shift_click_extends_across_the_document() {
    let (mut rt, board) = document();
    let a = rt.presented("first").unwrap().rect;
    let b = rt.presented("second").unwrap().rect;
    rt.pointer_down(a.left, a.top + 3.0);
    rt.pointer_up(a.left, a.top + 3.0);
    rt.set_modifiers(Modifiers::SHIFT);
    rt.pointer_down(b.right - 1.0, b.top + 3.0);
    rt.pointer_up(b.right - 1.0, b.top + 3.0);
    rt.set_modifiers(Modifiers::NONE);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello world\nsecond paragraph");
}

#[test]
fn empty_paragraphs_keep_their_separators_when_copied() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::col()
                .selectable()
                .child(el::text(""))
                .child(el::text("body").key("body"))
                .child(el::text(""))
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (240, 140),
        1.0,
    )
    .unwrap();
    let board = Board::default();
    rt.set_clipboard(board.clone());
    rt.click("body");
    rt.key_with(KeyCode::of('A').0, Modifiers::CTRL);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "\nbody\n");
}

#[test]
fn native_double_click_marker_selects_a_word_without_counting_a_third_press() {
    let (mut rt, board) = document();
    let r = rt.presented("first").unwrap().rect;
    rt.pointer_down(r.left + 2.0, r.top + 3.0);
    rt.pointer_up(r.left + 2.0, r.top + 3.0);
    // OS double-click timing may be longer than the runtime's fallback interval.
    rt.frame(Duration::from_millis(600));
    rt.pointer_down(r.left + 2.0, r.top + 3.0);
    rt.handle(&stillkit::Event::DoubleClick(
        stillkit::Pointer {
            at: stillkit::Point::new(r.left + 2.0, r.top + 3.0),
            screen: (0, 0),
        },
        stillkit::Button::Left,
    ));
    rt.pointer_up(r.left + 2.0, r.top + 3.0);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello");
    rt.pointer_down(r.left + 2.0, r.top + 3.0);
    rt.pointer_up(r.left + 2.0, r.top + 3.0);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "hello world");
}

#[test]
fn controls_added_after_selectable_exclude_their_labels() {
    let mut rt = Runtime::new(
        0u32,
        |_| {
            el::col().selectable().child(
                el::col()
                    .selectable()
                    .key("control")
                    .on_press(|s: &mut u32| *s += 1)
                    .child(el::text("Action").key("label")),
            )
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (240, 140),
        1.0,
    )
    .unwrap();
    assert!(rt.click("control"));
    rt.frame(Duration::ZERO);
    assert_eq!(*rt.state(), 1);
    assert_eq!(rt.presented("label").unwrap().selection, None);
}

#[test]
fn drag_targets_the_nearest_column() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::row()
                .selectable()
                .gap(20.0)
                .child(el::text("first").key("a").w(80.0))
                .child(el::text("second").key("b").w(80.0))
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (240, 140),
        1.0,
    )
    .unwrap();
    let board = Board::default();
    rt.set_clipboard(board.clone());
    rt.frame(Duration::ZERO);
    let a = rt.presented("a").unwrap().rect;
    let b = rt.presented("b").unwrap().rect;
    rt.pointer_down(a.left, a.top + 3.0);
    rt.pointer_move(b.right - 1.0, b.top + 3.0);
    rt.pointer_up(b.right - 1.0, b.top + 3.0);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "first\nsecond");
}
