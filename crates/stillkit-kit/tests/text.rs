//! Real Runtime editing, keyboard selection, semantics and focus restoration.
#![cfg(target_os = "windows")]
#[path = "../examples/kit_text.rs"]
#[allow(dead_code)]
mod gallery;
use gallery::State;
use std::time::Duration;
use stillkit::Semantics;
use stillkit::el::{El, Runtime};
fn rt(view: fn(&State) -> El<State>) -> Runtime<State> {
    Runtime::new(
        State::default(),
        view,
        gallery::themes().remove(0).1,
        (480, 400),
        1.,
    )
    .unwrap()
}
fn frame(rt: &mut Runtime<State>) {
    rt.frame(Duration::from_millis(16));
}
fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.tree()
        .into_iter()
        .find(|n| n.focused)
        .and_then(|n| n.key.map(|s| s.to_string()))
}
#[test]
fn typing_clear_and_escape() {
    let mut r = rt(gallery::search);
    r.tab(false);
    r.type_text("hello");
    frame(&mut r);
    assert_eq!(r.state().value, "hello");
    r.click("search/clear");
    frame(&mut r);
    assert_eq!(r.state().value, "");
    let mut r = rt(gallery::search);
    r.tab(false);
    r.type_text("again");
    frame(&mut r);
    r.key(0x1b);
    frame(&mut r);
    assert_eq!(r.state().value, "");
    assert_ne!(focused(&r).as_deref(), Some("search"));
}
#[test]
fn secure_reveal() {
    let mut r = rt(gallery::secure);
    r.tab(false);
    r.type_text("secret");
    frame(&mut r);
    assert!(!r.tree().iter().any(|n| n.text.as_deref() == Some("secret")));
    r.click("secure/reveal");
    frame(&mut r);
    assert!(r.state().revealed);
    assert!(r.tree().iter().any(|n| n.text.as_deref() == Some("secret")));
    r.click("secure/reveal");
    frame(&mut r);
    assert!(!r.state().revealed);
}
#[test]
fn error_description() {
    let r = rt(|_| {
        stillkit_kit::TextField::new("Email")
            .error("Bad email")
            .build()
    });
    assert!(r.semantics().iter().any(|(_,s)|matches!(s,Semantics::Detail(d) if d.description.as_deref()==Some("Invalid: Bad email"))));
}
#[test]
fn combobox_filter_keys_pick() {
    let mut r = rt(gallery::combo);
    r.tab(false);
    r.type_text("de");
    frame(&mut r);
    assert!(r.state().combo.open);
    assert!(r.find("combo/option-0").is_some());
    assert!(r.find("combo/option-2").is_none());
    r.key(0x28);
    frame(&mut r);
    assert_eq!(r.state().combo.highlighted, 1);
    r.key(0x0d);
    frame(&mut r);
    assert_eq!(r.state().combo.selected, Some(1));
    assert_eq!(r.state().combo.query, "Development");
    assert!(!r.state().combo.open);
}
#[test]
fn palette_filter_run_return() {
    let mut r = rt(gallery::palette);
    r.click("commands");
    frame(&mut r);
    assert_eq!(focused(&r).as_deref(), Some("palette/input"));
    r.type_text("folder");
    frame(&mut r);
    assert!(r.find("new").is_none());
    r.key(0x0d);
    frame(&mut r);
    assert_eq!(r.state().ran, 10);
    assert!(!r.state().palette.open);
    assert_eq!(focused(&r).as_deref(), Some("commands"));
    r.click("commands");
    frame(&mut r);
    r.key(0x1b);
    frame(&mut r);
    assert_eq!(focused(&r).as_deref(), Some("commands"));
}
#[test]
fn select_typeahead() {
    let mut r = rt(gallery::select);
    r.tab(false);
    r.type_text("l");
    frame(&mut r);
    assert_eq!(r.state().selected, 2);
    r.key(0x0d);
    frame(&mut r);
    r.type_text("m");
    frame(&mut r);
    assert_eq!(r.state().selected, 1);
}
#[test]
fn max_length_counter() {
    let mut r = rt(gallery::area);
    r.tab(false);
    r.type_text("abcdefghijklmno");
    frame(&mut r);
    assert_eq!(r.state().value, "abcdefghijkl");
    assert!(
        r.tree()
            .iter()
            .any(|n| n.text.as_deref() == Some("12 / 12"))
    );
}
#[test]
fn sheets() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/media/kit");
    for (name, theme) in gallery::themes() {
        stillkit::golden::check(
            &gallery::sheet(theme)?,
            &dir.join(format!("text-{name}.png")),
        )?;
    }
    Ok(())
}

#[test]
fn disabled_and_readonly_reject_edits() {
    for disabled in [false, true] {
        let mut r = Runtime::new(
            State::default(),
            move |s| {
                stillkit_kit::TextField::new("Locked")
                    .key("locked")
                    .value(s.value.as_str())
                    .disabled(disabled)
                    .read_only(!disabled)
                    .on_input(|s: &mut State, t| s.value = t.into())
                    .build()
            },
            gallery::themes().remove(0).1,
            (400, 200),
            1.,
        )
        .unwrap();
        r.tab(false);
        r.type_text("ignored");
        frame(&mut r);
        assert_eq!(r.state().value, "");
    }
}
#[test]
fn palette_arrows_and_empty_results() {
    let mut r = rt(gallery::palette);
    r.click("commands");
    frame(&mut r);
    r.key(0x28);
    frame(&mut r);
    assert_eq!(r.state().palette.highlighted, 1);
    r.key(0x0d);
    frame(&mut r);
    assert_eq!(r.state().ran, 10);
    r.click("commands");
    frame(&mut r);
    r.type_text("nothing matches");
    frame(&mut r);
    r.key(0x28);
    r.key(0x0d);
    frame(&mut r);
    assert!(r.state().palette.open);
    assert_eq!(r.state().ran, 10);
    assert!(
        r.tree()
            .iter()
            .any(|n| n.text.as_deref() == Some("No commands found"))
    );
}
#[test]
fn reduced_motion_clear_presence_settles() {
    let mut r = rt(gallery::search);
    r.set_reduced_motion(true);
    r.tab(false);
    r.type_text("x");
    frame(&mut r);
    assert!(r.find("search/clear").is_some());
    r.click("search/clear");
    frame(&mut r);
    r.frame(Duration::from_secs(8));
    assert!(!r.animating());
    assert!(r.find("search/clear").is_none());
}
