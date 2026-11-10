//! Interaction feedback and focus control, headless: washes and focus rings easing on the
//! runtime clock (and disarming), focus and blur by key, focus-within looks, invalid and
//! active-descendant semantics, and cells whose hits let presses through to their row.

use std::time::Duration;

use stillkit::el::{El, KeyCode, Runtime, col, el, row, text, text_input, token};
use stillkit::{Detail, HitId, Op, Rect, Rgba, Role, Scheme, Semantics, Style, Theme};

const FRAME: Duration = Duration::from_millis(16);

fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}

#[derive(Default)]
struct S {
    pressed: Vec<&'static str>,
    query: String,
}

fn button(key: &'static str) -> El<S> {
    el().key(key).label(key).w(80.0).h(28.0).radius(6.0).on_press(move |s: &mut S| s.pressed.push(key))
}

fn runtime(view: fn(&S) -> El<S>) -> Runtime<S> {
    Runtime::new(S::default(), view, theme(), (320, 240), 1.0).unwrap()
}

/// The fill painted exactly over `rect` with no radius other than the button's: its wash
/// (the buttons here have no fill of their own).
fn wash(rt: &Runtime<S>, rect: Rect) -> Rgba {
    rt.list()
        .ops
        .iter()
        .find_map(|op| match op {
            Op::Fill { rect: r, color, .. } if *r == rect => Some(*color),
            _ => None,
        })
        .expect("a pressable box always draws its wash")
}

/// The focus ring around `rect`: the stroke drawn just outside it.
fn ring(rt: &Runtime<S>, rect: Rect) -> (Rect, Rgba) {
    rt.list()
        .ops
        .iter()
        .find_map(|op| match op {
            Op::Stroke { rect: r, color, .. } if r.left < rect.left && r.right > rect.right && r.top < rect.top && (r.top - rect.top).abs() < 8.0 => Some((*r, *color)),
            _ => None,
        })
        .expect("a pressable box always draws its ring")
}

fn two_buttons(_: &S) -> El<S> {
    col().pad(10.0).gap(10.0).child(button("a")).child(button("b"))
}

#[test]
fn a_hover_wash_eases_in_and_out_on_the_clock_then_disarms() {
    let mut rt = runtime(two_buttons);
    let a = rt.find("a").unwrap();
    let hover = rt.theme().color.hover;
    rt.pointer_move(a.center().x, a.center().y);
    assert!(rt.frame(FRAME));
    let first = wash(&rt, a);
    assert!(first.a > 0.0 && first.a < hover.a, "the first frame is on the way in: {first:?}");
    assert!(rt.animating(), "frames keep coming while it eases");
    let mut alphas = vec![first.a];
    while rt.animating() {
        assert!(alphas.len() < 60, "it settles");
        rt.frame(FRAME);
        alphas.push(wash(&rt, a).a);
    }
    assert!(alphas.windows(2).all(|w| w[0] <= w[1]), "it eases in without overshoot: {alphas:?}");
    assert_eq!(wash(&rt, a), hover);
    assert_eq!(rt.view_runs(), 1, "the view never runs for feedback");
    // Settled: no frames, nothing painted.
    assert!(!rt.frame(FRAME));

    // Pressing eases to the pressed wash, releasing back out.
    rt.pointer_down(a.center().x, a.center().y);
    rt.frame(FRAME);
    let mid = wash(&rt, a);
    assert!(mid != hover && mid != rt.theme().color.pressed, "{mid:?}");
    rt.pointer_up(a.center().x, a.center().y);
    rt.pointer_move(300.0, 230.0);
    for _ in 0..60 {
        rt.frame(FRAME);
    }
    assert_eq!(wash(&rt, a), Rgba::TRANSPARENT);
    assert!(!rt.animating(), "nothing is left easing");
    assert_eq!(rt.wake_at(), None, "and nothing waits");
    assert_eq!(rt.state().pressed, ["a"]);
}

#[test]
fn reduced_motion_lands_washes_and_rings_at_once() {
    let mut rt = runtime(two_buttons);
    rt.set_reduced_motion(true);
    let a = rt.find("a").unwrap();
    rt.pointer_move(a.center().x, a.center().y);
    rt.frame(FRAME);
    assert_eq!(wash(&rt, a), rt.theme().color.hover);
    rt.tab(false);
    rt.frame(FRAME);
    let (_, color) = ring(&rt, a);
    assert_eq!(color.a, rt.theme().focus_ring.opacity);
    assert!(!rt.animating());
}

#[test]
fn the_focus_ring_fades_and_grows_in_to_where_it_rests() {
    let mut rt = runtime(two_buttons);
    let a = rt.find("a").unwrap();
    rt.tab(false);
    rt.frame(FRAME);
    let (early, color) = ring(&rt, a);
    let opacity = rt.theme().focus_ring.opacity;
    assert!(color.a > 0.0 && color.a < opacity, "{color:?}");
    while rt.animating() {
        rt.frame(FRAME);
    }
    let (rest, color) = ring(&rt, a);
    assert!(rest.width() > early.width(), "it grew out from the box: {early:?} -> {rest:?}");
    assert_eq!(color, rt.theme().color.accent.with_alpha(opacity));
    // At rest it is the theme's ring: its gap and width outside the box.
    let r = rt.theme().focus_ring;
    assert_eq!(rest, a.inflate(r.gap + r.width / 2.0));
}

/// A search field whose Escape blurs it and whose `/` (on the root) focuses it; Down moves
/// to the results and Up from the first result back to the field.
fn search(s: &S) -> El<S> {
    col()
        .pad(10.0)
        .gap(10.0)
        .on_char(|_, c, tx| {
            if c == '/' {
                tx.focus("query");
            }
            (c == '/').into()
        })
        .child(
            col().key("box").pad(4.0).focus_within(|el| el.fill(token::ACCENT)).child(
                text_input("query")
                    .w(200.0)
                    .value(s.query.as_str())
                    .on_input(|s: &mut S, t| s.query = t.into())
                    .invalid(s.query.contains('!'))
                    .description("No exclamation marks")
                    .active_descendant("result-1")
                    .on_key(|_, key, tx| match key {
                        KeyCode::ESCAPE => {
                            tx.blur();
                            stillkit::el::Handled::Yes
                        }
                        KeyCode::DOWN => {
                            tx.focus("result-0");
                            stillkit::el::Handled::Yes
                        }
                        _ => stillkit::el::Handled::No,
                    }),
            ),
        )
        .child(
            col().key("results").role(Role::ListBox).label("Results").children((0..3).map(|i| {
                el().key(format!("result-{i}")).role(Role::ListBoxOption).label(format!("Result {i}")).w(200.0).h(24.0).focusable()
            })),
        )
        .child(button("disabled").disabled(true))
}

fn hit_of(rt: &Runtime<S>, label: &str) -> HitId {
    rt.list().hits.iter().find(|h| &*h.label == label).map(|h| h.id).unwrap()
}

fn focused_key(rt: &Runtime<S>) -> Option<String> {
    rt.tree().into_iter().find(|n| n.focused).and_then(|n| n.key.map(|k| k.to_string()))
}

#[test]
fn handlers_and_the_host_focus_and_blur_by_key() {
    let mut rt = runtime(search);
    rt.type_text("/");
    rt.frame(FRAME);
    assert_eq!(focused_key(&rt).as_deref(), Some("query"), "a character handler focused the field");
    rt.key(KeyCode::DOWN.0);
    rt.frame(FRAME);
    assert_eq!(focused_key(&rt).as_deref(), Some("result-0"));
    assert!(rt.list().focus.is_some());
    // A key moved it there, so its ring shows.
    while rt.animating() {
        rt.frame(FRAME);
    }
    let row = rt.find("result-0").unwrap();
    assert!(ring(&rt, row).1.a > 0.0);

    rt.focus("query");
    rt.frame(FRAME);
    assert_eq!(focused_key(&rt).as_deref(), Some("query"));
    rt.key(KeyCode::ESCAPE.0);
    rt.frame(FRAME);
    assert_eq!(focused_key(&rt), None, "Escape's handler blurred it");
    // Nothing focusable by that key: focus stays where it is.
    rt.focus("result-2");
    rt.frame(FRAME);
    rt.focus("disabled");
    rt.frame(FRAME);
    rt.focus("results");
    rt.frame(FRAME);
    assert_eq!(focused_key(&rt).as_deref(), Some("result-2"));
    rt.blur();
    rt.frame(FRAME);
    assert_eq!(focused_key(&rt), None);
    assert!(rt.view_runs() <= 4, "focus requests don't run the view: {}", rt.view_runs());
}

#[test]
fn focus_within_restyles_its_box_without_running_the_view() {
    let mut rt = runtime(search);
    let fill = |rt: &Runtime<S>| rt.presented("box").unwrap().fill;
    assert_eq!(fill(&rt), None);
    rt.focus("query");
    rt.frame(FRAME);
    let views = rt.view_runs();
    assert_eq!(fill(&rt), Some(rt.theme().color.accent), "focus inside it");
    rt.focus("result-0");
    rt.frame(FRAME);
    assert_eq!(fill(&rt), None, "focus left it");
    rt.focus("query");
    rt.frame(FRAME);
    assert_eq!(fill(&rt), Some(rt.theme().color.accent));
    assert_eq!(rt.view_runs(), views, "the view did not run for it");
}

#[test]
fn invalid_and_the_active_descendant_reach_the_tree_and_assistive_technology() {
    let mut rt = runtime(search);
    let field = rt.presented("query").unwrap();
    assert!(!field.invalid);
    assert_eq!(field.active_descendant.as_deref(), Some("result-1"));
    let detail = |rt: &Runtime<S>| {
        let id = rt.list().hits.iter().find(|h| h.role == Role::TextInput).unwrap().id;
        rt.semantics()
            .iter()
            .find_map(|(h, s)| match s {
                Semantics::Detail(d) if *h == id => Some(d.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(detail(&rt).active_descendant, Some(hit_of(&rt, "Result 1")));
    assert!(!detail(&rt).invalid);

    rt.focus("query");
    rt.frame(FRAME);
    rt.type_text("hi!");
    rt.frame(FRAME);
    assert!(rt.presented("query").unwrap().invalid);
    assert_eq!(
        detail(&rt),
        Detail {
            description: Some("No exclamation marks".into()),
            invalid: true,
            active_descendant: Some(hit_of(&rt, "Result 1")),
            ..Detail::default()
        }
    );
    let json = rt.tree_json();
    assert!(json.contains("\"invalid\":true,\"active_descendant\":\"result-1\""), "{json}");
}

/// A table row (pressable) whose cells are announced on their own.
fn table(_: &S) -> El<S> {
    col().pad(10.0).child(
        row()
            .key("row")
            .role(Role::Row)
            .label("Alice, 31")
            .on_press(|s: &mut S| s.pressed.push("row"))
            .child(text("Alice").key("name").role(Role::Cell).w(100.0))
            .child(text("31").key("age").role(Role::Cell).w(60.0))
            .child(button("edit")),
    )
}

#[test]
fn a_cells_hit_lets_presses_through_to_its_row_and_stays_announced() {
    let mut rt = runtime(table);
    let cells: Vec<&str> = rt.list().hits.iter().filter(|h| h.role == Role::Cell).map(|h| &*h.label).collect();
    assert_eq!(cells, ["Alice", "31"], "cells keep their own hits");
    assert!(rt.click("name"));
    rt.frame(FRAME);
    assert!(rt.click("age"));
    rt.frame(FRAME);
    // A control inside the row still takes its own press.
    assert!(rt.click("edit"));
    rt.frame(FRAME);
    assert_eq!(rt.state().pressed, ["row", "row", "edit"]);
    // Hover over a cell is the row's.
    let name = rt.find("name").unwrap().center();
    rt.pointer_move(name.x, name.y);
    rt.frame(FRAME);
    assert_eq!(rt.presented("row").unwrap().state, stillkit::interact::State::Hover);
}

/// A visible stroke drawn just outside `rect`: a focus ring showing around it.
fn ring_shown(rt: &Runtime<S>, rect: Rect) -> bool {
    rt.list().ops.iter().any(|op| match op {
        Op::Stroke { rect: r, color, .. } => r.left < rect.left && r.right > rect.right && (r.top - rect.top).abs() < 8.0 && color.a > 0.0,
        _ => false,
    })
}

fn search_well(quiet: bool) -> El<S> {
    row().key("well").pad(6.0).fill(Rgba::rgb8([40, 40, 40], 1.0)).focus_within(|e| e.fill(Rgba::rgb8([30, 50, 90], 1.0))).child(
        text_input("query").w(160.0).h(24.0).value("").focus_ring(!quiet).on_input(|s: &mut S, t| s.query = t.to_owned()),
    )
}

#[test]
fn a_field_inside_a_focus_within_box_can_leave_the_ring_to_it() {
    for quiet in [false, true] {
        let view = if quiet { |_: &S| col().pad(20.0).child(search_well(true)) } else { |_: &S| col().pad(20.0).child(search_well(false)) };
        let mut rt = runtime(view);
        rt.frame(Duration::ZERO);
        let field = rt.find("query").unwrap();
        rt.pointer_down(field.center().x, field.center().y);
        rt.pointer_up(field.center().x, field.center().y);
        for _ in 0..30 {
            rt.frame(FRAME);
        }
        assert_eq!(ring_shown(&rt, field), !quiet, "quiet {quiet}: the field's own ring");
    }
}

fn skipped(_: &S) -> El<S> {
    row().child(button("handle").skip_tab()).child(button("field")).child(button("next"))
}

#[test]
fn a_skipped_element_takes_the_pointer_but_not_a_tab_stop() {
    let mut rt = runtime(skipped);
    rt.frame(FRAME);
    let focused = |rt: &Runtime<S>| rt.tree().into_iter().find(|n| n.focused).and_then(|n| n.key).map(|k| k.to_string());
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("field"), "Tab passes the handle");
    rt.tab(false);
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("field"), "and wraps past it");
    let handle = rt.find("handle").unwrap();
    rt.pointer_move(handle.center().x, handle.center().y);
    rt.frame(FRAME);
    rt.pointer_down(handle.center().x, handle.center().y);
    rt.pointer_up(handle.center().x, handle.center().y);
    rt.frame(FRAME);
    assert_eq!(rt.state().pressed, ["handle"], "still pressed by the pointer");
}
