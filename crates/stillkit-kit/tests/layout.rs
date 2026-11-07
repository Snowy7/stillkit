#![cfg(windows)]
#[allow(dead_code)]
#[path = "../examples/layout_support/mod.rs"]
mod gallery;
use std::{sync::Arc, time::Duration};
use stillkit::el::{El, Runtime, col, text};
use stillkit::{AccessibilityAction, Semantics};
use stillkit_kit::*;
#[derive(Default)]
struct State {
    choice: usize,
    open: bool,
    value: f64,
    phase: u64,
    hidden: bool,
    sort: Option<(usize, SortDirection)>,
    selected: Arc<str>,
}
fn view(s: &State, case: usize) -> El<State> {
    match case {
        0 => Tabs::new("Pages", s.choice)
            .tab("First", text("First panel"))
            .tab("Second", text("Second panel"))
            .on_change(|s: &mut State, i| s.choice = i)
            .build(),
        1 => Disclosure::new("Details", s.open, text("Expanded body").h(60.))
            .on_toggle(|s: &mut State| s.open = !s.open)
            .build(),
        2 => Stepper::new("Copies", 0., 10., 1., s.value)
            .on_change(|s: &mut State, v| s.value = v)
            .build(),
        3 => Slider::new("Volume", s.value as f32)
            .step(0.1)
            .on_change(|s: &mut State, v| s.value = v.into())
            .build(),
        4 => RadioGroup::new("Quality", ["Low", "High", "Maximum"], s.choice)
            .on_change(|s: &mut State, i| s.choice = i)
            .build(),
        5 => Segmented::new("Period", ["Day", "Week", "Month"], s.choice)
            .on_change(|s: &mut State, i| s.choice = i)
            .build(),
        6 => {
            let mut t = Table::new("Projects", ["Name", "Status"])
                .height(90.)
                .row("alpha", ["Alpha", "Ready"], s.selected.as_ref() == "alpha")
                .row("beta", ["Beta", "Active"], s.selected.as_ref() == "beta")
                .row("gamma", ["Gamma", "Paused"], false)
                .on_sort(|s: &mut State, v| s.sort = Some(v))
                .on_select(|s: &mut State, k| s.selected = k);
            if let Some((i, d)) = s.sort {
                t = t.sorted(i, d);
            }
            t.build()
        }
        7 => Tree::new(
            "Files",
            vec![
                TreeNode::new("parent", "Project")
                    .expanded(s.open)
                    .child(TreeNode::new("child", "Source")),
                TreeNode::new("other", "Other"),
            ],
        )
        .on_expand(|s: &mut State, (_, open)| s.open = open)
        .on_select(|s: &mut State, k| s.selected = k)
        .build(),
        8 => Sidebar::new("Navigation")
            .section("Workspace")
            .item(
                "first",
                "First",
                Icon::Home,
                s.choice == 0,
                |s: &mut State| s.choice = 0,
            )
            .item(
                "second",
                "Second",
                Icon::Folder,
                s.choice == 1,
                |s: &mut State| s.choice = 1,
            )
            .build(),
        // Pinned: CI machines (Windows Server) run with animations off, which would freeze it.
        9 => Spinner::new("Loading", s.phase)
            .reduced_motion(false)
            .visible(!s.hidden)
            .on_tick(|s: &mut State| s.phase += 1)
            .build(),
        _ => Ring::new("Usage", 0.57).build(),
    }
}
fn runtime(case: usize) -> Runtime<State> {
    let mut r = Runtime::new(
        State::default(),
        move |s| col().pad(10.).child(view(s, case).key("target")),
        gallery::themes().remove(0).1,
        (400, 400),
        1.,
    )
    .unwrap();
    r.frame(Duration::ZERO);
    r
}
fn key(r: &mut Runtime<State>, code: u16) {
    r.key(code);
    r.frame(Duration::ZERO);
}
fn detail(r: &Runtime<State>, label: &str) -> stillkit::Detail {
    let id = r
        .list()
        .hits
        .iter()
        .find(|h| h.label.as_ref() == label)
        .unwrap()
        .id;
    r.semantics()
        .iter()
        .find_map(|(hit, s)| match s {
            Semantics::Detail(d) if *hit == id => Some(d.clone()),
            _ => None,
        })
        .unwrap()
}
#[test]
fn tabs_arrows_switch_panel_and_selected_semantics() {
    let mut r = runtime(0);
    r.tab(false);
    key(&mut r, 0x27);
    assert_eq!(r.state().choice, 1);
    assert!(r.find("panel-1").is_some());
    assert_eq!(detail(&r, "Second").selected, Some(true));
    key(&mut r, 0x27);
    assert_eq!(r.state().choice, 0);
}
#[test]
fn accordion_expands_with_height_motion_and_semantics() {
    let mut r = runtime(1);
    let before = r.find("target").unwrap();
    r.tab(false);
    key(&mut r, 0x20);
    assert!(r.state().open);
    assert_eq!(detail(&r, "Details").expanded, Some(true));
    assert!(r.find("target").unwrap().height() > before.height());
    assert!(r.animating());
}
#[test]
fn range_keys_rapid_input_clamp_and_assistive_values() {
    for case in [2, 3] {
        let mut r = runtime(case);
        r.tab(false);
        r.key(0x27);
        r.key(0x27);
        r.frame(Duration::ZERO);
        let expected = if case == 2 { 2. } else { 0.2 };
        assert!((r.state().value - expected).abs() < 0.00001);
        key(&mut r, 0x23);
        assert_eq!(r.state().value, if case == 2 { 10. } else { 1. });
        key(&mut r, 0x24);
        assert_eq!(r.state().value, 0.);
        let (id, min, max) = r
            .semantics()
            .iter()
            .find_map(|(id, s)| match s {
                Semantics::Range { min, max, .. } => Some((*id, *min, *max)),
                _ => None,
            })
            .unwrap();
        assert_eq!(min, 0.);
        assert_eq!(max, if case == 2 { 10. } else { 1. });
        r.accessibility_action(id, &AccessibilityAction::Increment);
        r.frame(Duration::ZERO);
        assert!(r.state().value > 0.);
    }
}
#[test]
fn radio_and_segmented_roving_selection() {
    for (case, arrow) in [(4, 0x28), (5, 0x27)] {
        let mut r = runtime(case);
        r.tab(false);
        key(&mut r, arrow);
        assert_eq!(r.state().choice, 1);
        key(&mut r, 0x23);
        assert_eq!(r.state().choice, 2);
        key(&mut r, arrow);
        assert_eq!(r.state().choice, 0);
        assert!(
            r.semantics()
                .iter()
                .any(|(_, s)| matches!(s,Semantics::Detail(d)if d.selected==Some(true)))
        );
    }
}
#[test]
fn table_sort_select_and_fixed_header() {
    let mut r = runtime(6);
    assert!(r.click("column-0"));
    r.frame(Duration::ZERO);
    assert_eq!(r.state().sort, Some((0, SortDirection::Ascending)));
    r.click("column-0");
    r.frame(Duration::ZERO);
    assert_eq!(r.state().sort, Some((0, SortDirection::Descending)));
    assert!(r.click("beta"));
    r.frame(Duration::ZERO);
    assert_eq!(r.state().selected.as_ref(), "beta");
    assert_eq!(detail(&r, "Beta, Active").selected, Some(true));
    let header = r.find("header").unwrap();
    let p = r.find("body").unwrap().center();
    r.wheel_at(p.x, p.y, -100);
    r.frame(Duration::from_secs(1));
    assert_eq!(r.find("header").unwrap(), header);
}
#[test]
fn tree_keyboard_expands_navigates_and_collapses() {
    let mut r = runtime(7);
    r.tab(false);
    key(&mut r, 0x27);
    assert!(r.state().open);
    assert_eq!(detail(&r, "Project").expanded, Some(true));
    key(&mut r, 0x28);
    key(&mut r, 0x0d);
    assert_eq!(r.state().selected.as_ref(), "child");
    key(&mut r, 0x26);
    key(&mut r, 0x25);
    assert!(!r.state().open);
    assert!(r.find("child").is_none());
}
#[test]
fn sidebar_keyboard_selection() {
    let mut r = runtime(8);
    r.tab(false);
    key(&mut r, 0x28);
    assert_eq!(r.state().choice, 1);
    assert_eq!(detail(&r, "Second").selected, Some(true));
}
#[test]
fn spinner_hidden_cancels_timeout_and_ticker() {
    let mut r = runtime(9);
    r.frame(Duration::from_millis(150));
    assert_eq!(r.state().phase, 1);
    r.update(|s| s.hidden = true);
    r.frame(Duration::ZERO);
    let phase = r.state().phase;
    for _ in 0..20 {
        r.frame(Duration::from_secs(1));
    }
    assert_eq!(r.state().phase, phase);
    assert!(!r.animating());
    assert!(!r.frame(Duration::from_secs(1)));
}
#[test]
fn meter_announces_range_and_value() {
    let r = runtime(10);
    assert!(r.semantics().iter().any(|(_,s)|matches!(s,Semantics::Range{value,min,max,..}if (*value-0.57).abs()<0.001 && *min==0. && *max==1.)));
    assert_eq!(detail(&r, "Usage").value.as_deref(), Some("57%"));
}
#[test]
fn layout_golden_sheets() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/media/kit");
    for (name, theme) in gallery::themes() {
        let actual = gallery::sheet(theme).unwrap();
        let path = out.join(format!("layout-{name}.png"));
        if std::env::var_os("STILLKIT_BLESS").is_some() {
            stillkit::golden::write(&actual, &path).unwrap();
        }
        let expected = stillkit::golden::read(&path).unwrap();
        assert!(
            actual.pixels == expected.pixels,
            "layout golden differs: {name}"
        );
    }
}

#[test]
fn reduced_spinner_and_disabled_ranges_stay_idle() {
    let mut r = Runtime::new(
        0_u64,
        |phase| {
            Spinner::new("Loading", *phase)
                .reduced_motion(true)
                .on_tick(|s: &mut u64| *s += 1)
                .build()
        },
        gallery::themes().remove(0).1,
        (100, 100),
        1.,
    )
    .unwrap();
    r.frame(Duration::ZERO);
    for _ in 0..3 {
        r.frame(Duration::from_secs(1));
    }
    assert_eq!(*r.state(), 0);
    assert!(!r.animating());
    let mut r = Runtime::new(
        0_f64,
        |v| {
            Stepper::new("Copies", 0., 10., 1., *v)
                .disabled(true)
                .on_change(|s: &mut f64, v| *s = v)
                .build()
        },
        gallery::themes().remove(0).1,
        (300, 100),
        1.,
    )
    .unwrap();
    r.frame(Duration::ZERO);
    r.tab(false);
    r.key(0x27);
    r.frame(Duration::ZERO);
    assert_eq!(*r.state(), 0.);
}

#[test]
fn tab_indicator_and_ring_spring_then_settle() {
    let mut r = runtime(0);
    let start = r.presented("indicator").unwrap().rect;
    r.tab(false);
    key(&mut r, 0x27);
    let end = r.find("indicator").unwrap();
    r.frame(Duration::from_millis(40));
    let mid = r.presented("indicator").unwrap().rect;
    assert!(mid.left > start.left && mid.left < end.left);
    for _ in 0..200 {
        if !r.animating() {
            break;
        }
        r.frame(Duration::from_millis(16));
    }
    assert!(!r.animating());
    let mut r = Runtime::new(
        0_f32,
        |v| Ring::new("Usage", *v).build(),
        gallery::themes().remove(0).1,
        (100, 100),
        1.,
    )
    .unwrap();
    r.frame(Duration::ZERO);
    r.update(|v| *v = 0.8);
    r.frame(Duration::ZERO);
    assert!(r.animating());
    for _ in 0..200 {
        if !r.animating() {
            break;
        }
        r.frame(Duration::from_millis(16));
    }
    assert!(!r.animating());
}
