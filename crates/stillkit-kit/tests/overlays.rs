//! Public kit overlay behaviour, with actual headless input and virtual-clock timers.
#![cfg(target_os = "windows")]
#[path = "../examples/kit_overlays.rs"]
#[allow(dead_code)]
mod gallery;
use gallery::State;
use std::time::Duration;
use stillkit::el::{El, Runtime};
use stillkit::{Detail, Role, Semantics};
fn runtime(view: fn(&State) -> El<State>) -> Runtime<State> {
    Runtime::new(
        State::default(),
        view,
        gallery::themes().remove(0).1,
        (480, 360),
        1.,
    )
    .unwrap()
}
fn frame(rt: &mut Runtime<State>) {
    rt.frame(Duration::ZERO);
}
fn settle(rt: &mut Runtime<State>) {
    for _ in 0..200 {
        rt.frame(Duration::from_millis(16));
        if !rt.animating() {
            return;
        }
    }
    panic!("overlay did not settle");
}
fn focus(rt: &Runtime<State>) -> Option<String> {
    rt.tree()
        .into_iter()
        .find(|n| n.focused)
        .and_then(|n| n.key.map(|k| k.to_string()))
}
fn detail(rt: &Runtime<State>, label: &str) -> Detail {
    let id = rt
        .list()
        .hits
        .iter()
        .find(|h| h.label.as_ref() == label)
        .unwrap()
        .id;
    rt.semantics()
        .iter()
        .find_map(|(i, s)| {
            if *i == id {
                if let Semantics::Detail(d) = s {
                    Some(d.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or_default()
}
#[test]
fn menu_navigation_skips_disabled_and_activation_closes_and_returns_focus() {
    let mut rt = runtime(gallery::menu);
    rt.click("menu-trigger");
    frame(&mut rt);
    assert!(rt.state().menu);
    assert_eq!(focus(&rt).as_deref(), Some("icons")); // checked member is the entry point
    assert!(rt.list().hits.iter().any(|h| h.role == Role::Menu));
    assert!(
        rt.list()
            .hits
            .iter()
            .any(|h| h.role == Role::MenuItemCheckBox && h.checked == Some(true))
    );
    assert!(
        rt.list()
            .hits
            .iter()
            .any(|h| h.role == Role::MenuItemRadio && h.checked == Some(true))
    );
    rt.key(0x23);
    frame(&mut rt);
    assert_eq!(focus(&rt).as_deref(), Some("sort"));
    rt.key(0x28);
    frame(&mut rt);
    assert_eq!(focus(&rt).as_deref(), Some("new"));
    rt.key(0x20);
    frame(&mut rt);
    assert_eq!(rt.state().actions, 1);
    assert!(!rt.state().menu);
    assert_eq!(focus(&rt).as_deref(), Some("menu-trigger"));
    rt.click("menu-trigger");
    frame(&mut rt);
    rt.key(0x0d);
    frame(&mut rt);
    assert_eq!(rt.state().actions, 2);
    rt.click("menu-trigger");
    frame(&mut rt);
    rt.key(0x1b);
    frame(&mut rt);
    assert!(!rt.state().menu);
    rt.click("menu-trigger");
    frame(&mut rt);
    rt.pointer_down(470., 350.);
    rt.pointer_up(470., 350.);
    frame(&mut rt);
    assert!(!rt.state().menu);
}
#[test]
fn context_menu_uses_point_and_menu_key() {
    let mut rt = runtime(gallery::context);
    let at = rt.find("context-trigger").unwrap().center();
    rt.right_click(at.x, at.y);
    frame(&mut rt);
    assert!(rt.state().context.is_some());
    rt.key(0x1b);
    frame(&mut rt);
    assert!(rt.state().context.is_none());
    rt.click("context-trigger");
    rt.key(0x5d);
    frame(&mut rt);
    assert!(rt.state().context.is_some());
    rt.key(0x0d);
    frame(&mut rt);
    assert!(rt.state().context.is_none());
    assert_eq!(rt.state().actions, 1);
}
#[test]
fn select_changes_value_matches_width_and_opens_from_keyboard() {
    let mut rt = runtime(gallery::select);
    assert_eq!(detail(&rt, "Size").value.as_deref(), Some("Small"));
    rt.tab(false);
    rt.key(0x28);
    frame(&mut rt);
    assert!(rt.state().select);
    assert_eq!(focus(&rt).as_deref(), Some("size/option-0"));
    rt.key(0x28);
    rt.key(0x0d);
    frame(&mut rt);
    assert_eq!(rt.state().value, 1);
    assert!(!rt.state().select);
    assert_eq!(detail(&rt, "Size").value.as_deref(), Some("Medium"));
    assert_eq!(focus(&rt).as_deref(), Some("size"));
    for key in [0x20, 0x0d] {
        rt.key(key);
        frame(&mut rt);
        assert!(rt.state().select);
        rt.key(0x1b);
        frame(&mut rt);
    }
}
#[test]
fn dialogs_focus_cancel_trap_and_return_with_modal_semantics() {
    for (view, trigger, prefix, is_alert) in [
        (
            gallery::dialog as fn(&State) -> El<State>,
            "dialog-trigger",
            "dialog",
            false,
        ),
        (gallery::alert, "alert-trigger", "alert", true),
    ] {
        let mut rt = runtime(view);
        rt.click(trigger);
        frame(&mut rt);
        assert_eq!(focus(&rt), Some(format!("{prefix}/cancel")));
        let label = if is_alert {
            "Delete this item?"
        } else {
            "Save changes?"
        };
        assert!(detail(&rt, label).modal);
        assert!(rt.list().hits.iter().any(|h| h.role
            == if is_alert {
                Role::AlertDialog
            } else {
                Role::Dialog
            }));
        rt.tab(false);
        assert_eq!(focus(&rt), Some(format!("{prefix}/confirm")));
        rt.tab(false);
        assert_eq!(focus(&rt), Some(format!("{prefix}/cancel")));
        rt.key(0x1b);
        frame(&mut rt);
        assert_eq!(focus(&rt).as_deref(), Some(trigger));
        rt.click(trigger);
        frame(&mut rt);
        rt.click(&format!("{prefix}/cancel"));
        frame(&mut rt);
        assert!(!rt.state().dialog && !rt.state().alert);
        assert_eq!(rt.state().actions, 0);
        rt.click(trigger);
        frame(&mut rt);
        rt.click(&format!("{prefix}/confirm"));
        frame(&mut rt);
        assert_eq!(rt.state().actions, 1);
        assert!(!rt.state().dialog && !rt.state().alert);
    }
}
#[test]
fn popover_closes_via_escape_and_trigger_toggle() {
    let mut rt = runtime(gallery::popover);
    rt.click("popover-trigger");
    frame(&mut rt);
    assert!(rt.state().popover);
    assert_eq!(focus(&rt).as_deref(), Some("popover-trigger"));
    rt.key(0x1b);
    frame(&mut rt);
    assert!(!rt.state().popover);
    rt.click("popover-trigger");
    frame(&mut rt);
    rt.click("popover-trigger");
    frame(&mut rt);
    assert!(!rt.state().popover);
}
#[test]
fn toast_timeout_removes_deadline_and_animation_work() {
    let mut rt = runtime(gallery::toast);
    assert_eq!(rt.wake_at(), None);
    rt.update(|s| s.toast = true);
    frame(&mut rt);
    assert!(
        rt.list()
            .hits
            .iter()
            .any(|h| h.role == Role::Status && h.label.as_ref() == "Your changes were saved")
    );
    assert!(rt.wake_at().is_some());
    rt.frame(Duration::from_secs(6));
    assert!(!rt.state().toast);
    settle(&mut rt);
    assert_eq!(rt.wake_at(), None);
    assert!(!rt.animating());
}
#[test]
fn tooltip_is_delayed_noninteractive_description_and_reduced_motion_settles() {
    let mut rt = runtime(gallery::tooltip);
    rt.set_reduced_motion(true);
    let p = rt.find("battery").unwrap().center();
    rt.pointer_move(p.x, p.y);
    rt.frame(Duration::from_millis(600));
    assert!(
        rt.tree()
            .iter()
            .any(|n| n.text.as_deref() == Some("Battery 82%"))
    );
    assert_eq!(
        detail(&rt, "Battery").description.as_deref(),
        Some("Battery 82%")
    );
    assert!(
        !rt.list()
            .hits
            .iter()
            .any(|h| h.label.as_ref() == "Battery 82%")
    );
    rt.pointer_move(470., 350.);
    frame(&mut rt);
    settle(&mut rt);
    assert_eq!(rt.wake_at(), None);
}
#[test]
fn golden_sheets() -> anyhow::Result<()> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/media/kit");
    for (name, theme) in gallery::themes() {
        stillkit::golden::check(
            &gallery::sheet(theme)?,
            &dir.join(format!("overlays-{name}.png")),
        )?;
    }
    Ok(())
}

#[test]
fn match_width_popup_surfaces_and_reduced_motion_use_runtime_contracts() {
    let mut rt = runtime(gallery::select);
    rt.click("size");
    frame(&mut rt);
    settle(&mut rt);
    let trigger = rt.find("size").unwrap();
    let menu = rt
        .list()
        .hits
        .iter()
        .find(|h| h.role == Role::Menu)
        .unwrap();
    assert!(
        menu.rect.width() >= trigger.width(),
        "menu {:?}, trigger {:?}",
        menu.rect,
        trigger
    );
    rt.key(0x1b);
    frame(&mut rt);
    settle(&mut rt);
    rt.set_reduced_motion(true);
    rt.click("size");
    frame(&mut rt);
    settle(&mut rt);
    assert!(!rt.animating());
    rt.key(0x1b);
    frame(&mut rt);
    settle(&mut rt);
    assert_eq!(rt.wake_at(), None);

    let mut rt = Runtime::new(
        State::default(),
        gallery::select,
        gallery::themes().remove(0).1,
        (200, 100),
        1.,
    )
    .unwrap();
    rt.set_screen(Some(stillkit::Rect::new(-300., -300., 1200., 900.)));
    rt.click("size");
    frame(&mut rt);
    assert_eq!(rt.popups().len(), 1);
    rt.key(0x1b);
    frame(&mut rt);
    settle(&mut rt);
    assert!(rt.popups().is_empty());
}

#[test]
fn disabled_select_has_value_but_cannot_open() {
    let mut rt = Runtime::new(
        State::default(),
        |s| {
            stillkit_kit::Select::new(
                "Size",
                ["Small", "Large"],
                s.value,
                s.select,
                |s: &mut State, open| s.select = open,
                |s, value| s.value = value,
            )
            .key("size")
            .disabled(true)
            .build()
        },
        gallery::themes().remove(0).1,
        (480, 360),
        1.,
    )
    .unwrap();
    rt.click("size");
    rt.tab(false);
    rt.key(0x20);
    rt.key(0x28);
    frame(&mut rt);
    assert!(!rt.state().select);
    assert_eq!(detail(&rt, "Size").value.as_deref(), Some("Small"));
}
