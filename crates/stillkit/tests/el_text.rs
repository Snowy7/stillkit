//! Text fields, headless: typing, selection, word movement, undo and paste through a test
//! clipboard; a controlled value the view refuses; Enter submitting and Tab leaving; a secure
//! field's accessibility; and the caret's blink, which never runs the view and stops.

use std::time::Duration;

use stillkit::el::{El, Handled, KeyCode, MemoryClipboard, Modifiers, Runtime, col, el, text_area, text_input};
use stillkit::{AccessibilityAction, Role, Scheme, Semantics, Style, Theme};

const FRAME: Duration = Duration::from_millis(16);
const CTRL: Modifiers = Modifiers::CTRL;
const SHIFT: Modifiers = Modifiers::SHIFT;
const CTRL_SHIFT: Modifiers = Modifiers { ctrl: true, shift: true, alt: false, meta: false };

fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

#[derive(Default)]
struct Form {
    name: String,
    /// Only digits get in.
    pin: String,
    password: String,
    notes: String,
    submitted: Vec<String>,
    /// Keys and characters that reached the form itself.
    bubbled: Vec<u16>,
    chars: Vec<char>,
}

fn view(s: &Form) -> El<Form> {
    col()
        .pad(8.0)
        .gap(8.0)
        .on_key(|s: &mut Form, key, _| {
            s.bubbled.push(key.0);
            Handled::No
        })
        .on_char(|s: &mut Form, c, _| {
            s.chars.push(c);
            Handled::Yes
        })
        .child(
            text_input("name")
                .w(200.0)
                .label("Name")
                .value(s.name.as_str())
                .on_input(|s: &mut Form, t| s.name = t.into())
                .on_submit(|s: &mut Form, t| s.submitted.push(t.into())),
        )
        .child(
            text_input("pin")
                .w(200.0)
                .placeholder("Digits")
                .value(s.pin.as_str())
                .on_input(|s: &mut Form, t| {
                    if t.chars().all(|c| c.is_ascii_digit()) {
                        s.pin = t.into();
                    }
                }),
        )
        .child(
            text_input("password")
                .w(200.0)
                .label("Password")
                .secure()
                .value(s.password.as_str())
                .on_input(|s: &mut Form, t| s.password = t.into()),
        )
        .child(
            text_area("notes")
                .w(200.0)
                .rows(2)
                .value(s.notes.as_str())
                .on_input(|s: &mut Form, t| s.notes = t.into()),
        )
        .child(el().key("after").label("After").focusable().w(20.0).h(20.0))
}

fn runtime() -> Runtime<Form> {
    let mut rt = Runtime::new(Form::default(), view, theme(), (240, 240), 1.0).unwrap();
    rt.set_clipboard(MemoryClipboard(Some("clip".into())));
    rt.frame(FRAME);
    rt
}

/// Focus on the field keyed `key` (a click), and a frame.
fn focus(rt: &mut Runtime<Form>, key: &str) {
    assert!(rt.click(key));
    rt.frame(FRAME);
}

fn shown(rt: &Runtime<Form>, key: &str) -> (String, (usize, usize)) {
    let s = rt.presented(key).unwrap();
    (s.text.as_deref().unwrap_or("").into(), s.selection.unwrap_or_default())
}

#[test]
fn typing_selection_words_undo_and_paste() {
    let mut rt = runtime();
    focus(&mut rt, "name");
    rt.type_text("hello big world");
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "hello big world", "on_input took every character");
    rt.key_with(KeyCode::LEFT.0, CTRL);
    rt.key(KeyCode::LEFT.0);
    rt.key_with(KeyCode::LEFT.0, CTRL | SHIFT);
    assert_eq!(shown(&rt, "name").1, (9, 6), "Ctrl+Shift+Left selects back to the word's start");
    rt.type_text("small");
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "hello small world");
    rt.key_with(KeyCode::HOME.0, Modifiers::NONE);
    rt.key_with(KeyCode::RIGHT.0, CTRL);
    assert_eq!(shown(&rt, "name").1, (6, 6), "Ctrl+Right lands on the next word's start");
    rt.key_with(KeyCode::BACKSPACE.0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "small world", "Ctrl+Backspace deletes the word");
    rt.key_with(KeyCode::of('Z').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "hello small world", "undo");
    rt.key_with(KeyCode::of('Z').0, CTRL_SHIFT);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "small world", "redo");
    rt.key_with(KeyCode::END.0, Modifiers::NONE);
    rt.key_with(KeyCode::of('V').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "small worldclip", "paste from the clipboard");
    rt.key_with(KeyCode::of('A').0, CTRL);
    rt.key_with(KeyCode::of('X').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "", "cut everything");
    rt.key_with(KeyCode::of('V').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "small worldclip", "what was cut pastes back");
    assert!(rt.state().bubbled.is_empty(), "the field used every key: {:?}", rt.state().bubbled);
    assert!(rt.state().chars.is_empty(), "and every character");
}

#[test]
fn double_and_triple_clicks_select_a_word_and_the_line() {
    let mut rt = runtime();
    focus(&mut rt, "name");
    rt.type_text("one two three");
    rt.frame(FRAME);
    let r = rt.find("name").unwrap();
    // Somewhere in "two": a third of the way along the drawn text.
    let (x, y) = (r.left + 1.0 + rt.presented("name").unwrap().rect.width() * 0.0, r.center().y);
    let two = (0..r.width() as i32)
        .map(|dx| x + dx as f32)
        .find(|&x| {
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(Duration::from_millis(600));
            shown(&rt, "name").1 .1 == 5
        })
        .expect("a click lands the caret inside two");
    rt.pointer_down(two, y);
    rt.pointer_up(two, y);
    rt.pointer_down(two, y);
    rt.pointer_up(two, y);
    assert_eq!(shown(&rt, "name").1, (4, 7), "a double click selects two");
    rt.pointer_down(two, y);
    rt.pointer_up(two, y);
    assert_eq!(shown(&rt, "name").1, (0, 13), "a triple click selects the line");
}

#[test]
fn a_refused_value_keeps_the_old_text() {
    let mut rt = runtime();
    focus(&mut rt, "pin");
    rt.type_text("12");
    rt.frame(FRAME);
    rt.type_text("x");
    rt.frame(FRAME);
    assert_eq!(rt.state().pin, "12");
    assert_eq!(shown(&rt, "pin"), ("12".into(), (2, 2)), "the field shows what the view kept");
    rt.type_text("3");
    rt.frame(FRAME);
    assert_eq!(rt.state().pin, "123", "and typing goes on from there");
    rt.key_with(KeyCode::of('Z').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().pin, "12", "undo skips the refused edit");
}

#[test]
fn enter_submits_tab_leaves_and_other_keys_bubble() {
    let mut rt = runtime();
    focus(&mut rt, "name");
    rt.type_text("Ada");
    rt.key(KeyCode::ENTER.0);
    rt.frame(FRAME);
    assert_eq!(rt.state().submitted, ["Ada"]);
    rt.key(KeyCode::DOWN.0);
    rt.key_with(KeyCode::of('S').0, CTRL);
    rt.key(KeyCode::of('Q').0);
    assert_eq!(rt.state().bubbled, [0x28, 0x53], "Down and Ctrl+S go on; a typing key doesn't");
    rt.tab(false);
    rt.frame(FRAME);
    rt.type_text("x");
    assert_eq!(rt.state().pin, "", "Tab left the name field");
    assert_eq!(rt.state().name, "Ada");
    // A text area keeps Enter as a new line, and lets Tab go.
    focus(&mut rt, "notes");
    rt.type_text("a");
    rt.key(KeyCode::ENTER.0);
    rt.type_text("b");
    rt.frame(FRAME);
    assert_eq!(rt.state().notes, "a\nb");
    rt.key(KeyCode::UP.0);
    rt.type_text("c");
    rt.frame(FRAME);
    assert_eq!(rt.state().notes, "ac\nb", "Up moves a line");
    rt.tab(false);
    rt.frame(FRAME);
    assert!(rt.presented("after").unwrap().focused, "Tab leaves a text area");
}

#[test]
fn a_secure_field_shows_bullets_and_tells_no_text() {
    let mut rt = runtime();
    focus(&mut rt, "password");
    rt.type_text("hunter2");
    rt.key_with(KeyCode::of('A').0, CTRL);
    rt.key_with(KeyCode::of('C').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().password, "hunter2");
    let hit = rt.list().hits.iter().find(|h| h.label.as_ref() == "Password").unwrap().clone();
    assert_eq!(hit.role, Role::PasswordInput);
    assert!(
        !rt.semantics().iter().any(|(id, s)| *id == hit.id && matches!(s, Semantics::Text(_))),
        "no text for assistive technology"
    );
    assert_eq!(rt.presented("password").unwrap().text, None, "nor in the tree dump");
    assert!(!format!("{:?}", rt.list().ops).contains("hunter2"), "nothing draws it");
    // Copy was refused: pasting into the name gives the old clipboard.
    focus(&mut rt, "name");
    rt.key_with(KeyCode::of('V').0, CTRL);
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "clip");
    // A plain field tells its text, and takes a new one from assistive technology.
    let name = rt.list().hits.iter().find(|h| h.label.as_ref() == "Name").unwrap().id;
    assert!(rt.semantics().iter().any(|(id, s)| *id == name && *s == Semantics::Text("clip".into())));
    let runs = rt.semantics().iter().find_map(|(id, s)| match s {
        Semantics::Field(t) if *id == name => Some(t.clone()),
        _ => None,
    });
    let runs = runs.expect("the field's lines, by character");
    assert_eq!(runs.lines.len(), 1);
    assert_eq!(&*runs.lines[0].text, "clip");
    assert_eq!(runs.lines[0].chars.len(), 4);
    assert!(runs.lines[0].chars.windows(2).all(|w| w[1].0 > w[0].0), "characters left to right");
    assert_eq!((runs.anchor, runs.caret), ((0, 4), (0, 4)), "the caret after the paste");
    assert!(
        !rt.semantics().iter().any(|(_, s)| matches!(s, Semantics::Field(t) if t.lines.iter().any(|l| l.text.contains('h')))),
        "the password has no runs"
    );
    assert!(rt.accessibility_action(name, &AccessibilityAction::SetText("Grace".into())));
    rt.frame(FRAME);
    assert_eq!(rt.state().name, "Grace");
}

#[test]
fn the_caret_blinks_without_the_view_and_stops() {
    let mut rt = runtime();
    assert_eq!(rt.wake_at(), None, "no timer while no field has focus");
    focus(&mut rt, "name");
    let views = rt.view_runs();
    let start = rt.now();
    let first = rt.wake_at().expect("the blink is the only timer");
    assert!(first > start && first - start <= Duration::from_millis(530), "{first:?}");
    let mut paints = rt.paints();
    let mut blinks = 0;
    while let Some(at) = rt.wake_at() {
        rt.frame(at - rt.now());
        if rt.paints() > paints {
            blinks += 1;
            paints = rt.paints();
        }
        assert!(rt.now() - start <= Duration::from_secs(6), "the blink stops");
    }
    assert!(blinks >= 8, "it blinked ({blinks})");
    assert_eq!(rt.view_runs(), views, "blinking never ran the view");
    assert!(
        rt.list().ops.iter().any(|op| format!("{op:?}").contains("Fill")),
        "the caret holds still, shown"
    );
    // Typing restarts it; leaving the field ends it.
    rt.type_text("a");
    rt.frame(FRAME);
    assert!(rt.wake_at().is_some());
    rt.tab(false);
    rt.frame(FRAME);
    rt.tab(false);
    rt.tab(false);
    rt.tab(false);
    rt.frame(FRAME);
    assert!(rt.presented("after").unwrap().focused);
    assert_eq!(rt.wake_at(), None, "no blink once no field has focus");
}
