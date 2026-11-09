#![cfg(windows)]
#[allow(dead_code)]
#[path = "../examples/app_shell.rs"]
mod editor;
use std::time::Duration;
use stillkit::el::{KeyCode, Modifiers, Runtime};
use stillkit::{Button, Event, Point, Pointer, Rect};
use stillkit_kit::*;

fn runtime() -> Runtime<editor::Model> {
    let mut rt = Runtime::new(
        editor::Model::default(),
        editor::view,
        editor::themes().remove(0).1,
        (1000, 720),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    rt
}
fn frame(rt: &mut Runtime<editor::Model>) {
    rt.frame(Duration::ZERO);
}
fn pointer(x: f32, y: f32) -> Pointer {
    Pointer {
        at: Point::new(x, y),
        screen: (x as i32, y as i32),
    }
}
#[test]
fn split_keyboard_clamps_collapses_restores_and_resets() {
    let mut rt = runtime();
    rt.focus("workspace/splitter");
    frame(&mut rt);
    for _ in 0..30 {
        rt.key(0x27);
        frame(&mut rt);
    }
    assert_eq!(rt.state().split.size, 400.);
    rt.key(0x0d);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 0.);
    rt.key(0x27);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 140.);
    rt.key_with(0x27, Modifiers::SHIFT);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 141.);
    rt.key(0x24);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 220.);
    rt.focus("console/splitter");
    frame(&mut rt);
    rt.key(0x28);
    frame(&mut rt);
    assert_eq!(rt.state().bottom.size, 310.);
}
#[test]
fn split_capture_resizes_outside_grip_cancel_rolls_back_and_double_click_resets() {
    let mut rt = runtime();
    let p = rt.find("workspace/splitter").unwrap().center();
    rt.pointer_down(p.x, p.y);
    frame(&mut rt);
    rt.pointer_move(p.x + 100., p.y + 40.);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 320.);
    rt.handle(&Event::CaptureLost);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 220.);
    let p = rt.find("workspace/splitter").unwrap().center();
    rt.pointer_down(p.x, p.y);
    frame(&mut rt);
    rt.pointer_move(p.x - 200., p.y);
    frame(&mut rt);
    rt.pointer_up(p.x - 200., p.y);
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 0.);
    let p = rt.find("workspace/splitter").unwrap().center();
    rt.handle(&Event::DoubleClick(pointer(p.x, p.y), Button::Left));
    frame(&mut rt);
    assert_eq!(rt.state().split.size, 220.);
}
#[test]
fn tabs_arrows_cycle_and_host_ctrl_tab_wraps() {
    let mut rt = runtime();
    rt.focus("documents/tab/source");
    frame(&mut rt);
    rt.key(0x27);
    frame(&mut rt);
    assert_eq!(rt.state().selected.as_ref(), "readme");
    rt.key(0x23);
    frame(&mut rt);
    assert_eq!(rt.state().selected.as_ref(), "config");
    let event = TabStripState::shortcut(
        &Event::Key(9),
        Modifiers::CTRL,
        &rt.state().tabs,
        &rt.state().selected,
    )
    .unwrap();
    rt.update(|s| editor::tab_event(s, event));
    frame(&mut rt);
    assert_eq!(rt.state().selected.as_ref(), "source");
    let event = TabStripState::shortcut(
        &Event::Key(9),
        Modifiers::CTRL | Modifiers::SHIFT,
        &rt.state().tabs,
        &rt.state().selected,
    )
    .unwrap();
    assert_eq!(event, TabEvent::Select("config".into()));
}
#[test]
fn captured_tab_reorders_then_tears_out_without_discarding_document() {
    let mut rt = runtime();
    let p = rt.find("documents/tab/source").unwrap();
    rt.pointer_down(p.left + 35., p.top + 10.);
    frame(&mut rt);
    rt.pointer_move(p.left + 240., p.top + 10.);
    frame(&mut rt);
    assert_eq!(rt.state().tabs[1].id.as_ref(), "source");
    rt.pointer_up(p.left + 240., p.bottom + 100.);
    frame(&mut rt);
    assert!(rt.state().status.contains("Move source"));
    assert_eq!(rt.state().tabs.len(), 3);
}
#[test]
fn middle_click_closes_and_dirty_close_remains_app_owned() {
    let mut rt = runtime();
    for (id, expected) in [("source", 3), ("readme", 2)] {
        let p = rt.find(&format!("documents/tab/{id}")).unwrap().center();
        rt.pointer_move(p.x, p.y);
        frame(&mut rt);
        rt.handle(&Event::Press(pointer(p.x, p.y), Button::Middle));
        frame(&mut rt);
        rt.handle(&Event::Release(pointer(p.x, p.y), Button::Middle));
        frame(&mut rt);
        assert_eq!(rt.state().tabs.len(), expected);
    }
}
#[test]
fn shortcut_modifier_keyup_cancel_clear_and_capture() {
    let mut s = ShortcutRecorderState::default();
    s.begin();
    assert!(s.handle(&Event::Key(0x11), Modifiers::CTRL));
    assert!(s.handle(&Event::KeyUp(0x11), Modifiers::NONE));
    s.handle(&Event::Key(0x4b), Modifiers::SHIFT);
    assert_eq!(
        s.chord,
        Some(KeyChord {
            key: KeyCode::of('K'),
            modifiers: Modifiers::SHIFT
        })
    );
    assert!(!s.recording);
    s.begin();
    s.handle(&Event::Key(0x1b), Modifiers::NONE);
    assert!(s.chord.is_some());
    s.begin();
    s.handle(&Event::Deactivated, Modifiers::NONE);
    assert!(!s.recording);
    s.begin();
    s.handle(&Event::Key(0x08), Modifiers::NONE);
    assert_eq!(s.chord, None);
}
#[test]
fn hover_delay_cancel_interactive_transfer_and_idle_deadline() {
    let mut s = HoverCardState::default();
    let anchor = Rect::new(10., 10., 100., 30.);
    let card = Rect::new(10., 44., 200., 100.);
    s.handle(
        &Event::Move(pointer(20., 20.)),
        anchor,
        None,
        1.,
        Duration::ZERO,
    );
    assert!(!s.advance(Duration::from_millis(499)));
    assert!(s.advance(Duration::from_millis(500)));
    assert!(s.open);
    assert_eq!(s.deadline(), None);
    assert!(!s.handle(
        &Event::Move(pointer(20., 80.)),
        anchor,
        Some(card),
        1.,
        Duration::from_secs(1)
    ));
    assert!(s.open);
    assert!(s.handle(
        &Event::Move(pointer(500., 500.)),
        anchor,
        Some(card),
        1.,
        Duration::from_secs(1)
    ));
    s.enter(Duration::from_secs(2));
    s.handle(&Event::Leave, anchor, None, 1., Duration::from_secs(2));
    assert!(!s.advance(Duration::from_secs(3)));
    assert_eq!(s.deadline(), None);
}

#[test]
fn hover_card_panel_padding_remains_interactive() {
    let mut rt = runtime();
    rt.update(|s| s.hover.open = true);
    frame(&mut rt);
    let panel = rt.find("about-panel").unwrap();
    let anchor = rt.find("about").unwrap();
    let mut hover = rt.state().hover.clone();
    assert!(!hover.handle(
        &Event::Move(pointer(panel.left + 1., panel.top + 1.)),
        anchor,
        Some(panel),
        1.,
        Duration::ZERO
    ));
    assert!(hover.open);
}
#[test]
fn sidebar_collapse_removes_rows_and_count_is_announced() {
    let mut rt = runtime();
    let hit = rt
        .list()
        .hits
        .iter()
        .find(|h| h.label.as_ref() == "Project, 3 items")
        .unwrap();
    let p = hit.rect.center();
    rt.pointer_down(p.x, p.y);
    rt.pointer_up(p.x, p.y);
    frame(&mut rt);
    assert!(!rt.state().expanded);
    assert!(
        !rt.list()
            .hits
            .iter()
            .any(|h| h.label.as_ref() == "Documentation")
    );
}
#[test]
fn all_looks_layout_and_settle_without_idle_wakes() {
    for (name, theme) in editor::themes() {
        let mut rt = Runtime::new(
            editor::Model::default(),
            editor::view,
            theme,
            (1000, 720),
            1.,
        )
        .unwrap();
        rt.frame(Duration::ZERO);
        assert_eq!(rt.find("titlebar").unwrap().height(), 40., "{name}");
        assert!(rt.find("shortcut").unwrap().bottom <= 720., "{name}");
        rt.update(|s| s.status = "Changed".into());
        frame(&mut rt);
        rt.frame(Duration::from_secs(5));
        assert!(!rt.animating(), "{name}");
        assert_eq!(rt.wake_at(), None, "{name}");
    }
}

#[test]
fn popup_list_skips_disabled_rows_and_picks_by_keyboard() {
    let mut disabled = PopupItem::new("disabled", "Unavailable");
    disabled.disabled = true;
    let mut rt = Runtime::new(
        String::new(),
        move |selected: &String| {
            PopupList::new(
                "completion",
                "Files",
                selected.clone(),
                |s: &mut String, id| *s = id.to_string(),
            )
            .item(PopupItem::new("one", "First"))
            .item(PopupItem {
                id: disabled.id.clone(),
                label: disabled.label.clone(),
                description: None,
                disabled: true,
            })
            .item(PopupItem::new("two", "Second"))
            .build()
        },
        editor::themes().remove(0).1,
        (300, 240),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    rt.focus("completion/one");
    rt.frame(Duration::ZERO);
    rt.key(0x28);
    rt.frame(Duration::ZERO);
    rt.key(0x0d);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state(), "two");
}

#[test]
fn titlebar_caption_buttons_have_keyboard_actions_and_device_bounds() {
    let mut rt = runtime();
    // The native backend treats every semantic hit as client content. The band must
    // leave caption gaps without a covering semantic container.
    let caption_gap = Point::new(1., 20.);
    assert!(
        !rt.list()
            .hits
            .iter()
            .any(|hit| hit.rect.contains(caption_gap))
    );
    assert_eq!(rt.find("titlebar/close").unwrap().right, 1000.);
    rt.focus("titlebar/maximize");
    frame(&mut rt);
    rt.key(0x0d);
    frame(&mut rt);
    assert_eq!(rt.state().action, Some(WindowAction::Maximize));
    rt.set_scale(2.);
    rt.resize((2000, 1440));
    frame(&mut rt);
    assert_eq!(rt.find("titlebar").unwrap().height(), 80.);
    assert_eq!(rt.find("titlebar/close").unwrap().width(), 92.);
}
