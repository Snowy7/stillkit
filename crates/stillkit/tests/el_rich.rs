use std::time::Duration;
use stillkit::el::{self, KeyCode, Modifiers, Run, Runtime};
use stillkit::{Scheme, Theme};
#[derive(Clone, Default)]
struct Board(std::rc::Rc<std::cell::RefCell<String>>);
impl el::Clipboard for Board {
    fn get(&mut self) -> Option<String> {
        Some(self.0.borrow().clone())
    }
    fn set(&mut self, value: &str) {
        *self.0.borrow_mut() = value.into();
    }
}
fn theme() -> Theme {
    Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}
#[test]
fn runs_wrap_copy_plain_text_and_hold_still() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::rich_text([
                Run::new("regular "),
                Run::new("bold ").bold(),
                Run::new("italic ").italic(),
                Run::new("code ").code(),
                Run::new("underlined ").underline(),
                Run::new("strike").strike(),
            ])
            .key("rich")
            .w(95.0)
            .selectable()
        },
        theme(),
        (180, 240),
        1.0,
    )
    .unwrap();
    let board = Board::default();
    rt.set_clipboard(board.clone());
    rt.frame(Duration::ZERO);
    let rect = rt.presented("rich").unwrap().rect;
    assert!(rect.height() > 40.0, "runs wrap: {rect:?}");
    rt.click("rich");
    rt.key_with(KeyCode::of('A').0, Modifiers::CTRL);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("rich").unwrap().selection, Some((0, 42)));
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(
        &*board.0.borrow(),
        "regular bold italic code underlined strike"
    );
    let views = rt.view_runs();
    rt.frame(Duration::from_secs(20));
    assert_eq!(rt.view_runs(), views);
}
#[test]
fn link_click_dispatches_destination_and_exposes_hand_cursor() {
    let mut rt = Runtime::new(
        String::new(),
        |_| {
            el::rich_text([Run::new("link").link("app:docs")])
                .key("link")
                .selectable()
                .on_link(|s: &mut String, url| *s = url.into())
        },
        theme(),
        (180, 80),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let r = rt.presented("link").unwrap().rect;
    rt.pointer_move(r.left + 5.0, r.top + 10.0);
    assert_eq!(rt.cursor(), stillkit::Cursor::Hand);
    rt.pointer_down(r.left + 5.0, r.top + 10.0);
    rt.pointer_up(r.left + 5.0, r.top + 10.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state(), "app:docs");
}
#[cfg(all(windows, feature = "golden"))]
#[test]
fn rich_runs_golden() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::col().pad(16.0).fill(el::token::SURFACE).child(
                el::rich_text([
                    Run::new("Native rich text ").bold().size(19.0),
                    Run::new("italic ").italic(),
                    Run::new("underline ").underline(),
                    Run::new("deleted ").strike(),
                    Run::new("let answer = 42;").code(),
                    Run::new("\nRead the documentation").link("app:docs"),
                ])
                .w(230.0)
                .selectable(),
            )
        },
        theme(),
        (270, 160),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/rich-runs.png");
    stillkit::golden::check(&rt.snap().unwrap(), &path).unwrap();
}

#[test]
fn dragging_from_a_link_selects_without_opening_it() {
    let mut rt = Runtime::new(
        0usize,
        |_| {
            el::rich_text([Run::new("follow this link").link("app:docs")])
                .key("link")
                .selectable()
                .on_link(|s: &mut usize, _| *s += 1)
        },
        theme(),
        (200, 80),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let r = rt.presented("link").unwrap().rect;
    rt.pointer_down(r.left + 1.0, r.top + 10.0);
    rt.pointer_move(r.left + 40.0, r.top + 10.0);
    rt.pointer_up(r.left + 40.0, r.top + 10.0);
    rt.frame(Duration::ZERO);
    assert_eq!(*rt.state(), 0);
    assert!(
        rt.presented("link")
            .unwrap()
            .selection
            .is_some_and(|(a, b)| a != b)
    );
}

#[test]
fn linked_runs_expose_the_link_role() {
    let mut rt = Runtime::new(
        (),
        |_| el::rich_text([Run::new("documentation").link("app:docs")]).on_link(|_, _| {}),
        theme(),
        (200, 80),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    assert!(
        rt.list()
            .hits
            .iter()
            .any(|h| h.role == stillkit::Role::Link && h.actionable)
    );
}

#[test]
fn a_wrapped_link_keeps_one_identity_and_its_complete_label() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::rich_text([Run::new("Read all the documentation").link("app:docs")])
                .w(90.0)
                .on_link(|_, _| {})
        },
        theme(),
        (180, 160),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let links: Vec<_> = rt
        .list()
        .hits
        .iter()
        .filter(|h| h.role == stillkit::Role::Link)
        .collect();
    assert!(links.len() > 1);
    assert!(
        links
            .iter()
            .all(|h| h.id == links[0].id && &*h.label == "Read all the documentation")
    );
}

#[test]
fn a_link_clipped_out_of_its_viewport_does_not_change_the_cursor() {
    let mut rt = Runtime::new(
        (),
        |_| {
            el::col().child(
                el::col()
                    .w(150.0)
                    .h(50.0)
                    .absolute(0.0, 30.0)
                    .scroll(el::Axis::Vertical)
                    .child(
                        el::rich_text([Run::new("clipped link").link("app:docs")])
                            .absolute(0.0, -20.0)
                            .on_link(|_, _| {}),
                    ),
            )
        },
        theme(),
        (200, 100),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    rt.pointer_move(5.0, 15.0);
    assert_eq!(rt.cursor(), stillkit::Cursor::Arrow);
}
