//! Keys, focus groups, ranges, accessibility values, themed focus rings and arcs, headless:
//! roving Tab and wrapping arrows that skip disabled members, range stepping, key handlers
//! bubbling, what `tree_json` and the accessibility side table carry, and what paint draws.

use std::f32::consts::TAU;
use std::time::Duration;

use stillkit::el::{ArcStart, Axis, El, KeyCode, Runtime, arc, col, el, row, text};
use stillkit::theme::FocusRing;
use stillkit::{AccessibilityAction, Detail, Op, Rgba, Role, Scheme, Semantics, Style, Theme};

const FRAME: Duration = Duration::from_millis(16);

fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

#[derive(Default)]
struct Form {
    mode: usize,
    level: f64,
    pressed: Vec<&'static str>,
    deleted: usize,
}

/// A button, a radio group of four (the third disabled), a slider and another button.
fn view(s: &Form) -> El<Form> {
    let option = |i: usize| {
        el().key(format!("mode-{i}"))
            .role(Role::RadioButton)
            .label(format!("Mode {i}"))
            .w(40.0)
            .h(24.0)
            .checked(s.mode == i)
            .disabled(i == 2)
            .on_press(move |s: &mut Form| s.mode = i)
    };
    col()
        .gap(8.0)
        .pad(8.0)
        .on_key(|s: &mut Form, key, _| {
            if key == KeyCode::DELETE {
                s.deleted += 1;
            }
            (key == KeyCode::DELETE).into()
        })
        .child(button("first"))
        .child(
            row()
                .key("mode")
                .role(Role::RadioGroup)
                .label("Mode")
                .focus_group(Axis::Horizontal)
                .select_follows_focus()
                .children((0..4).map(option)),
        )
        .child(
            el().key("level")
                .label("Level")
                .w(200.0)
                .h(24.0)
                .range(0.0, 100.0, 5.0, s.level)
                .value(format!("{}%", s.level))
                .description("How loud")
                .on_step(|s: &mut Form, delta| s.level += delta),
        )
        .child(button("last").expanded(false).selected(true))
}

fn button(key: &'static str) -> El<Form> {
    el().key(key)
        .w(60.0)
        .h(24.0)
        .on_press(move |s: &mut Form| s.pressed.push(key))
        .child(text(key))
}

fn runtime() -> Runtime<Form> {
    Runtime::new(Form { mode: 1, ..Form::default() }, view, theme(), (300, 200), 1.0).unwrap()
}

fn focused(rt: &Runtime<Form>) -> Option<String> {
    rt.tree().into_iter().find(|s| s.focused).and_then(|s| s.key).map(|k| k.to_string())
}

fn key(rt: &mut Runtime<Form>, key: KeyCode) {
    rt.key(key.0);
    rt.frame(FRAME);
}

#[test]
fn tab_enters_a_group_once_on_its_selected_member_and_arrows_rove_and_wrap() {
    let mut rt = runtime();
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("first"));
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("mode-1"), "Tab lands on the checked option");
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("level"), "and leaves the group in one stop");
    rt.tab(true);
    assert_eq!(focused(&rt).as_deref(), Some("mode-1"), "Shift+Tab comes back to it");
    assert_eq!(rt.view_runs(), 1, "focus never runs the view");

    // Right skips the disabled option, presses where it lands (selection follows focus).
    key(&mut rt, KeyCode::RIGHT);
    assert_eq!(focused(&rt).as_deref(), Some("mode-3"));
    assert_eq!(rt.state().mode, 3);
    key(&mut rt, KeyCode::RIGHT);
    assert_eq!(focused(&rt).as_deref(), Some("mode-0"), "wraps to the first");
    key(&mut rt, KeyCode::LEFT);
    assert_eq!(focused(&rt).as_deref(), Some("mode-3"), "and back");
    key(&mut rt, KeyCode::HOME);
    assert_eq!((focused(&rt).as_deref(), rt.state().mode), (Some("mode-0"), 0));
    key(&mut rt, KeyCode::END);
    assert_eq!((focused(&rt).as_deref(), rt.state().mode), (Some("mode-3"), 3));
    // Across the group's axis an arrow moves focus on, as everywhere.
    key(&mut rt, KeyCode::DOWN);
    assert_eq!(focused(&rt).as_deref(), Some("level"));
    // Tab back into the group finds the new selection.
    rt.tab(true);
    assert_eq!(focused(&rt).as_deref(), Some("mode-3"));
}

#[test]
fn a_range_steps_by_keys_and_assistive_technology_within_its_ends() {
    let mut rt = runtime();
    rt.tab(false);
    rt.tab(false);
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("level"));
    key(&mut rt, KeyCode::RIGHT);
    assert_eq!(rt.state().level, 5.0);
    key(&mut rt, KeyCode::UP);
    key(&mut rt, KeyCode::PAGE_UP);
    assert_eq!(rt.state().level, 20.0, "a page is a tenth of the range");
    key(&mut rt, KeyCode::DOWN);
    assert_eq!(rt.state().level, 15.0);
    key(&mut rt, KeyCode::END);
    assert_eq!(rt.state().level, 100.0);
    key(&mut rt, KeyCode::RIGHT);
    assert_eq!(rt.state().level, 100.0, "never past the end");
    // Keys faster than frames add up from the latest value.
    rt.key(KeyCode::HOME.0);
    rt.key(KeyCode::RIGHT.0);
    rt.key(KeyCode::RIGHT.0);
    rt.frame(FRAME);
    assert_eq!(rt.state().level, 10.0);
    let hit = rt.list().hits.iter().find(|h| &*h.label == "Level").unwrap().id;
    assert!(rt.accessibility_action(hit, &AccessibilityAction::Increment));
    assert!(rt.accessibility_action(hit, &AccessibilityAction::SetNumber(250.0)));
    rt.frame(FRAME);
    assert_eq!(rt.state().level, 100.0, "set values clamp too");
    assert_eq!(focused(&rt).as_deref(), Some("level"), "stepping keeps the focus");
}

#[test]
fn key_handlers_bubble_and_unused_keys_go_on() {
    let mut rt = runtime();
    rt.tab(false);
    key(&mut rt, KeyCode::DELETE);
    assert_eq!(rt.state().deleted, 1, "the column's handler took it from its child");
    assert_eq!(rt.view_runs(), 2, "a used key re-runs the view");
    key(&mut rt, KeyCode::ENTER);
    assert_eq!(rt.state().pressed, ["first"], "unused keys still press");
    // With nothing focused the root still hears keys.
    let mut rt = runtime();
    key(&mut rt, KeyCode::DELETE);
    assert_eq!(rt.state().deleted, 1);
}

#[test]
fn values_reach_the_tree_dump_and_the_accessibility_table() {
    let mut rt = runtime();
    rt.update(|s| s.level = 40.0);
    rt.frame(FRAME);
    let json = rt.tree_json();
    let level = json.lines().find(|l| l.contains("\"key\":\"level\"")).unwrap();
    assert!(level.contains("\"role\":\"Slider\""), "{level}");
    assert!(level.contains("\"range\":[0,100,40]"), "{level}");
    assert!(level.contains("\"value\":\"40%\"") && level.contains("\"description\":\"How loud\""), "{level}");
    let last = json.lines().find(|l| l.contains("\"key\":\"last\"")).unwrap();
    assert!(last.contains("\"selected\":true") && last.contains("\"expanded\":false"), "{last}");
    let hit = |label: &str| rt.list().hits.iter().find(|h| &*h.label == label).unwrap().id;
    let of = |id| rt.semantics().iter().filter(move |(h, _)| *h == id).map(|(_, s)| s.clone()).collect::<Vec<_>>();
    assert_eq!(
        of(hit("Level")),
        [
            Semantics::Range { value: 40.0, min: 0.0, max: 100.0, step: 5.0 },
            Semantics::Detail(Detail {
                description: Some("How loud".into()),
                value: Some("40%".into()),
                selected: None,
                expanded: None,
            }),
        ]
    );
    assert_eq!(
        of(hit("last")),
        [Semantics::Detail(Detail { selected: Some(true), expanded: Some(false), ..Detail::default() })]
    );
    // A group with a role and label is announced, enabled, but no stop of its own.
    let group = rt.list().hits.iter().find(|h| &*h.label == "Mode").unwrap();
    assert!(group.enabled && !group.actionable && group.role == Role::RadioGroup);
}

/// The focused element's ring: the stroke right after its children.
fn ring(rt: &Runtime<Form>) -> (f32, Rgba) {
    rt.list()
        .ops
        .iter()
        .find_map(|op| match op {
            Op::Stroke { width, color, .. } if color.a > 0.0 => Some((*width, *color)),
            _ => None,
        })
        .expect("a visible ring")
}

#[test]
fn focus_rings_follow_the_theme_and_the_default_matches_the_kit() {
    let mut rt = runtime();
    rt.tab(false);
    rt.frame(FRAME);
    let accent = rt.theme().color.accent;
    // The default ring is the kit's `Frame::focus_ring`: 2 px, 2 px out, 0.9 of the accent.
    let rect = rt.find("first").unwrap();
    let kit = {
        let theme = rt.theme().clone();
        let ix = stillkit::Interaction {
            focus: rt.list().focus,
            focus_visible: true,
            ..Default::default()
        };
        let ui = rt.ui();
        let mut f = stillkit::Frame::new(&mut ui.text, &theme, 1.0, &ix);
        f.focus_ring(ix.focus.unwrap(), rect, 0.0);
        f.list.ops.pop().unwrap()
    };
    assert!(rt.list().ops.contains(&kit), "{kit:?}");
    assert_eq!(ring(&rt), (2.0, accent.with_alpha(0.9)));
    rt.set_theme(theme().focus_ring(FocusRing { width: 3.0, gap: 1.0, opacity: 0.5 }));
    rt.frame(FRAME);
    assert_eq!(rt.view_runs(), 1, "a theme restyles without the view");
    let (width, color) = ring(&rt);
    assert_eq!((width, color.a), (3.0, 0.5));
    let stroke = rt.list().ops.iter().find_map(|op| match op {
        Op::Stroke { rect: r, color, .. } if color.a > 0.0 => Some(*r),
        _ => None,
    });
    assert_eq!(stroke, Some(rect.inflate(1.0 + 1.5)), "its gap and width from the theme");
}

#[derive(Default)]
struct Meter(f32);

fn meter(s: &Meter) -> El<Meter> {
    col()
        .child(arc(s.0).key("ring").size(40.0).stroke(4.0))
        .child(arc(s.0).key("half").size(40.0).stroke(4.0).arc_start(ArcStart::Left).arc_span(0.5).animate())
}

fn arcs(ops: &[Op], out: &mut Vec<f32>) {
    for op in ops {
        match op {
            Op::Arc { sweep, .. } => out.push(*sweep),
            Op::Clipped { ops, .. } => arcs(ops, out),
            _ => {}
        }
    }
}

#[test]
fn arcs_draw_a_track_and_a_value_from_their_start() {
    let mut rt = Runtime::new(Meter(0.25), meter, theme(), (40, 80), 1.0).unwrap();
    let mut sweeps = Vec::new();
    arcs(&rt.list().ops, &mut sweeps);
    // The ring: a whole track and a quarter from twelve. The half meter from nine o'clock:
    // its track round to three (the whole circle clipped to the top-left quarter, then the
    // quarter past twelve), its value an eighth of a turn (an arc to half past ten, clipped to
    // the top-left quarter).
    let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
    assert!(near(sweeps[0], TAU) && near(sweeps[1], TAU / 4.0), "{sweeps:?}");
    assert!(near(sweeps[2], TAU) && near(sweeps[3], TAU / 4.0), "{sweeps:?}");
    assert!(near(sweeps[4], TAU * 7.0 / 8.0) && sweeps.len() == 5, "{sweeps:?}");
    let image = rt.snap().unwrap();
    let lit = |x: u32, y: u32| image.pixel(x, y)[3] > 0;
    // Half meter, centred at (20, 60), radius 18: lit left and top, not right or bottom.
    assert!(lit(3, 60) && lit(20, 43) && lit(37, 60), "the meter runs from nine over twelve to three");
    assert!(!lit(20, 78), "nothing below the meter");
    // The value springs when it has motion; settled, it sits on its target.
    rt.update(|s| s.0 = 1.0);
    rt.frame(FRAME);
    assert!(rt.animating());
    while rt.animating() {
        rt.frame(FRAME);
    }
    let mut sweeps = Vec::new();
    arcs(&rt.list().ops, &mut sweeps);
    assert!(sweeps.iter().filter(|s| near(**s, TAU)).count() >= 3, "{sweeps:?}");
}
