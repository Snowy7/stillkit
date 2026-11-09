//! Markdown uses one display-selection document while code controls keep their input.
#![cfg(feature = "markdown")]
use std::{cell::RefCell, rc::Rc, time::Duration};
use stillkit::el::{self, Clipboard, KeyCode, Modifiers, Runtime};
use stillkit::{Scheme, Theme};
use stillkit_kit::Markdown;
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
fn theme() -> Theme {
    Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}
#[test]
fn copy_selection_crosses_blocks_and_excludes_code_button() {
    let mut rt = Runtime::new(
        (),
        |_| Markdown::new("# Title\n\n**Body** and `code`.\n\n```rust\nlet n = 1;\n```\n").build(),
        theme(),
        (360, 400),
        1.0,
    )
    .unwrap();
    let board = Board::default();
    rt.set_clipboard(board.clone());
    rt.frame(Duration::ZERO);
    rt.pointer_down(4.0, 10.0);
    rt.pointer_up(4.0, 10.0);
    rt.key_with(KeyCode::of('A').0, Modifiers::CTRL);
    rt.key_with(KeyCode::of('C').0, Modifiers::CTRL);
    assert_eq!(&*board.0.borrow(), "Title\nBody and code.\nlet n = 1;\n");
}
#[test]
fn fenced_code_copy_preserves_source_and_link_activation_is_app_owned() {
    let mut rt = Runtime::new(
        String::new(),
        |_| {
            Markdown::new("[Guide](app:guide)\n\n```\na\n  b\n```\n")
                .on_copy(|s: &mut String, code| *s = code.into())
                .on_link(|s, url| *s = url.into())
                .build()
        },
        theme(),
        (360, 300),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let hit = rt
        .list()
        .hits
        .iter()
        .find(|h| h.actionable && &*h.label == "Copy code")
        .unwrap()
        .rect;
    rt.pointer_down(hit.left + 2.0, hit.top + 2.0);
    rt.pointer_up(hit.left + 2.0, hit.top + 2.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state(), "a\n  b\n");
    rt.pointer_move(3.0, 10.0);
    assert_eq!(rt.cursor(), stillkit::Cursor::Hand);
    rt.pointer_down(3.0, 10.0);
    rt.pointer_up(3.0, 10.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state(), "app:guide");
    let views = rt.view_runs();
    rt.frame(Duration::from_secs(10));
    assert_eq!(rt.view_runs(), views);
}
#[cfg(windows)]
#[test]
fn markdown_layout_goldens() {
    const SOURCE: &str = "# Native Markdown\n\nA paragraph with **bold**, *italic*, `code` and a [link](app:guide).\n\n3. Ordered item\n   - Nested item that wraps across a narrow column\n4. Next item\n\n> A quote with **emphasis**.\n\n---\n\n| Left | Right |\n| :--- | ---: |\n| Longer wrapping cell | Value |\n\n```rust\nlet answer = 42;\n```\n";
    for (name, scheme, width, scale) in [
        ("narrow-dark", Scheme::Dark, 340, 1.0),
        ("wide-light", Scheme::Light, 620, 1.5),
    ] {
        let mut rt = Runtime::new(
            (),
            |_| {
                el::col()
                    .pad(16.0)
                    .fill(el::token::SURFACE)
                    .child(Markdown::new(SOURCE).build())
            },
            Theme::solid(scheme, stillkit::theme::SYSTEM_BLUE),
            (width, (800.0 * scale) as u32),
            scale,
        )
        .unwrap();
        rt.frame(Duration::ZERO);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/golden/markdown-{name}.png"));
        stillkit::golden::check(&rt.snap().unwrap(), &path).unwrap();
    }
}

#[test]
fn changing_the_look_restyles_retained_markdown_without_parsing_again() {
    let mut rt = Runtime::new(
        (),
        |_| Markdown::new("# Heading\n\nText and `code`.").build(),
        theme(),
        (360, 200),
        1.0,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let views = rt.view_runs();
    let before = rt.list().ops.clone();
    rt.set_theme(Theme::one_ui(Scheme::Light, stillkit::theme::SYSTEM_BLUE));
    rt.frame(Duration::ZERO);
    assert_eq!(rt.view_runs(), views);
    assert_ne!(rt.list().ops, before);
}
