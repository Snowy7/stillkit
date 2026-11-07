//! Anchored overlays through the headless runtime: placement and flips, dismissal, focus
//! trap and return, the tooltip's delay, toast timers disarming, accessibility, and hover
//! never running the view.

#[path = "../examples/el_overlays.rs"]
#[allow(dead_code)]
mod overlays;

use std::time::Duration;

use overlays::{Demo, TOAST, theme, view};

use stillkit::el::{Align, El, HasPopup, Runtime, Side, col, el, popover};
use stillkit::{Detail, Event, Rect, Semantics};

fn demo() -> Runtime<Demo> {
    Runtime::new(Demo::default(), view, theme(), (480, 320), 1.0).unwrap()
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Frames until nothing moves.
fn settle<S>(rt: &mut Runtime<S>) {
    for _ in 0..200 {
        rt.frame(ms(16));
        if !rt.animating() {
            return;
        }
    }
    panic!("still animating");
}

fn focused<S>(rt: &Runtime<S>) -> Option<String> {
    rt.tree().into_iter().find(|s| s.focused).and_then(|s| s.key.map(|k| k.to_string()))
}

fn labels<S>(rt: &Runtime<S>) -> Vec<String> {
    rt.list().hits.iter().map(|h| h.label.to_string()).collect()
}

fn detail<S>(rt: &Runtime<S>, label: &str) -> Detail {
    let hit = rt.list().hits.iter().find(|h| &*h.label == label).unwrap().id;
    rt.semantics()
        .iter()
        .find_map(|(id, s)| match s {
            Semantics::Detail(d) if *id == hit => Some(d.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// A trigger at `at` with a popover open on `side`, `align`ed.
struct Edge {
    at: (f32, f32),
    side: Side,
    align: Align,
    wide: bool,
}

fn edge(s: &Edge) -> El<Edge> {
    let panel = el().key("p").w(120.0).h(60.0);
    let mut pop = popover(panel).side(s.side).align(s.align);
    if s.wide {
        pop = pop.match_width();
    }
    el().child(
        el().key("t")
            .absolute(s.at.0, s.at.1)
            .w(if s.wide { 200.0 } else { 40.0 })
            .h(20.0)
            .on_press(|_: &mut Edge| {})
            .overlay(pop),
    )
}

#[test]
fn overlays_open_on_their_side_and_flip_or_shift_to_stay_inside() {
    let place = |at, side, align, wide| {
        let mut rt = Runtime::new(Edge { at, side, align, wide }, edge, theme(), (480, 320), 1.0).unwrap();
        rt.frame(Duration::ZERO);
        (rt.find("t").unwrap(), rt.find("p").unwrap())
    };
    let (t, p) = place((100.0, 20.0), Side::Bottom, Align::Start, false);
    assert_eq!((p.left, p.top), (t.left, t.bottom + 6.0), "below, leading edges lined up");
    let (t, p) = place((100.0, 290.0), Side::Bottom, Align::Start, false);
    assert_eq!(p.bottom, t.top - 6.0, "no room below: it opens above");
    let (t, p) = place((430.0, 20.0), Side::Bottom, Align::Start, false);
    assert_eq!(p.right, t.right, "no room on the right: the trailing edges line up");
    let (_, p) = place((0.0, 100.0), Side::Right, Align::Center, false);
    assert_eq!(p.left, 46.0, "to the right of the anchor");
    let (_, p) = place((2.0, 20.0), Side::Bottom, Align::Center, false);
    assert_eq!(p.left, 8.0, "centred under a trigger at the edge, shifted inside the margin");
    let (t, p) = place((100.0, 20.0), Side::Bottom, Align::Start, true);
    assert_eq!(p.width(), t.width(), "as wide as its anchor");

    // A context menu at the pointer near the bottom-right corner opens up and to the left.
    let mut rt = demo();
    rt.right_click(440.0, 290.0);
    rt.frame(Duration::ZERO);
    let m = rt.find("menu").unwrap();
    assert_eq!((m.right, m.bottom), (440.0, 290.0), "{m:?}");
    rt.right_click(100.0, 100.0);
    rt.frame(Duration::ZERO);
    let m = rt.find("menu").unwrap();
    assert!(m.right <= 440.0, "a press outside a menu only closes it: {m:?}");
}

#[test]
fn escape_presses_outside_deactivation_and_a_lost_anchor_dismiss() {
    let mut rt = demo();
    rt.click("share");
    rt.frame(Duration::ZERO);
    assert!(rt.state().share && rt.find("share-panel").is_some());
    rt.key(0x1B);
    rt.frame(Duration::ZERO);
    assert!(!rt.state().share, "Escape");

    // A press outside a popover closes it and still lands.
    rt.click("share");
    rt.frame(Duration::ZERO);
    rt.click("copy");
    rt.frame(Duration::ZERO);
    assert!(!rt.state().share && rt.state().toasts.len() == 1, "outside, passed through");

    // The trigger toggles it closed (no dismissal reopening it).
    rt.click("share");
    rt.frame(Duration::ZERO);
    rt.click("share");
    rt.frame(Duration::ZERO);
    assert!(!rt.state().share, "the trigger closes it");

    // A press outside a menu only closes it.
    rt.right_click(200.0, 200.0);
    rt.frame(Duration::ZERO);
    assert!(rt.state().menu.is_some());
    rt.click("copy");
    rt.frame(Duration::ZERO);
    assert!(rt.state().menu.is_none() && rt.state().toasts.len() == 1, "outside, kept");

    // The window losing activation closes a popover.
    rt.click("share");
    rt.frame(Duration::ZERO);
    rt.handle(&Event::Deactivated);
    rt.frame(Duration::ZERO);
    assert!(!rt.state().share, "deactivated");

    // An anchor that goes, and a trigger that goes, dismiss.
    struct Gone {
        anchor: bool,
        trigger: bool,
        a: bool,
        b: bool,
    }
    let view = |s: &Gone| {
        let mut root = col::<Gone>()
            .overlay(popover(el().key("pa").w(40.0).h(40.0)).anchor("a").open(s.a).on_dismiss(|s: &mut Gone| s.a = false));
        if s.anchor {
            root = root.child(el().key("a").w(20.0).h(20.0));
        }
        if s.trigger {
            root = root.child(el().key("b").w(20.0).h(20.0).overlay(
                popover(el().key("pb").w(40.0).h(40.0)).open(s.b).on_dismiss(|s: &mut Gone| s.b = false),
            ));
        }
        root
    };
    let state = Gone {
        anchor: true,
        trigger: true,
        a: true,
        b: true,
    };
    let mut rt = Runtime::new(state, view, theme(), (200, 200), 1.0).unwrap();
    assert!(rt.find("pa").is_some() && rt.find("pb").is_some());
    rt.update(|s| (s.anchor, s.trigger) = (false, false));
    rt.frame(Duration::ZERO);
    assert!(!rt.state().a && !rt.state().b, "both dismissed in the same frame");
    settle(&mut rt);
    assert!(rt.find("pa").is_none() && rt.find("pb").is_none());
}

#[test]
fn menus_and_dialogs_take_focus_trap_it_and_give_it_back() {
    let mut rt = demo();
    // A popover takes no focus.
    rt.click("share");
    rt.frame(Duration::ZERO);
    assert_eq!(focused(&rt).as_deref(), Some("share"));
    rt.key(0x1B);
    rt.frame(Duration::ZERO);

    rt.click("delete");
    rt.frame(Duration::ZERO);
    assert_eq!(focused(&rt).as_deref(), Some("cancel"), "into the dialog");
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("ok"));
    rt.tab(false);
    rt.frame(Duration::ZERO);
    assert_eq!(focused(&rt).as_deref(), Some("cancel"), "Tab wraps inside the dialog");
    rt.tab(true);
    rt.frame(Duration::ZERO);
    assert_eq!(focused(&rt).as_deref(), Some("ok"));
    rt.key(0x1B);
    rt.frame(Duration::ZERO);
    assert!(!rt.state().confirm);
    assert_eq!(focused(&rt).as_deref(), Some("delete"), "back to the trigger");

    // From the keyboard: the Menu key opens the focused canvas's menu on its first item,
    // arrows move through the items, Escape goes back.
    rt.click("canvas");
    rt.key(0x5D);
    rt.frame(Duration::ZERO);
    let (canvas, menu) = (rt.find("canvas").unwrap(), rt.find("menu").unwrap());
    assert_eq!((menu.left, menu.top), (canvas.center().x, canvas.center().y), "at its centre");
    assert_eq!(focused(&rt).as_deref(), Some("cut"));
    rt.key(0x28);
    rt.frame(Duration::ZERO);
    assert_eq!(focused(&rt).as_deref(), Some("copy-item"));
    rt.key(0x26);
    rt.key(0x26);
    rt.frame(Duration::ZERO);
    assert_eq!(focused(&rt).as_deref(), Some("paste"), "wrapping");
    rt.key(0x0D);
    rt.frame(Duration::ZERO);
    assert!(rt.state().menu.is_none() && rt.state().toasts.last().unwrap().1 == "Pasted");
    assert_eq!(focused(&rt).as_deref(), Some("canvas"), "back where it was");
}

#[test]
fn tooltips_open_after_their_delay_on_hover_or_focus_without_running_the_view() {
    let mut rt = demo();
    let runs = rt.view_runs();
    let copy = rt.find("copy").unwrap().center();
    let shows = |rt: &Runtime<Demo>| rt.tree().iter().any(|s| s.text.as_deref() == Some("Copy the link"));
    rt.pointer_move(copy.x, copy.y);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.wake_at(), Some(rt.now() + stillkit::el::TOOLTIP_DELAY));
    rt.frame(ms(450));
    assert!(!shows(&rt), "not before its delay");
    rt.frame(ms(60));
    assert!(shows(&rt), "after it");
    assert_eq!(rt.wake_at(), None);
    assert!(!labels(&rt).iter().any(|l| l == "Copy the link"), "no hit of its own");
    assert_eq!(detail(&rt, "Copy").description.as_deref(), Some("Copy the link"));
    // Moving off closes it (it fades out); moving across other things arms nothing.
    rt.pointer_move(300.0, 200.0);
    rt.frame(Duration::ZERO);
    assert!(!shows(&rt));
    settle(&mut rt);
    assert_eq!(rt.wake_at(), None);
    assert_eq!(rt.view_runs(), runs, "hover and tooltips never ran the view");

    // Keyboard focus arriving shows it too; Escape hides it, and it stays hidden.
    rt.pointer_move(300.0, 300.0);
    rt.tab(false);
    rt.tab(false);
    assert_eq!(focused(&rt).as_deref(), Some("copy"));
    rt.frame(ms(600));
    assert!(shows(&rt));
    rt.key(0x1B);
    rt.frame(ms(600));
    assert!(!shows(&rt) && focused(&rt).as_deref() == Some("copy"), "Escape keeps the focus");
    assert_eq!(rt.wake_at(), None);
}

#[test]
fn toast_timers_fire_once_and_disarm() {
    let mut rt = demo();
    assert_eq!(rt.wake_at(), None);
    rt.click("copy");
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().toasts.len(), 1);
    let due = rt.now() + TOAST;
    assert_eq!(rt.wake_at(), Some(due));
    rt.frame(TOAST - ms(100));
    rt.click("copy");
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().toasts.len(), 2);
    assert_eq!(rt.wake_at(), Some(due), "the first toast is due first");
    rt.frame(ms(150));
    assert_eq!(rt.state().toasts.len(), 1, "the first timed out");
    rt.frame(TOAST);
    assert!(rt.state().toasts.is_empty());
    settle(&mut rt);
    assert_eq!(rt.wake_at(), None, "nothing armed once they are gone");
    assert!(!rt.animating(), "no ticker when idle");
    assert_eq!(rt.find("toasts").map(|r| r.height()), Some(0.0));
}

#[test]
fn triggers_announce_what_they_open_and_a_dialog_hides_the_background() {
    let mut rt = demo();
    let share = detail(&rt, "Share");
    assert_eq!((share.popup, share.expanded, share.controls), (Some(HasPopup::Dialog), Some(false), None));
    rt.click("share");
    rt.frame(Duration::ZERO);
    let panel = rt.list().hits.iter().find(|h| &*h.label == "Share link").unwrap();
    assert_eq!(panel.role, stillkit::Role::Dialog);
    let share = detail(&rt, "Share");
    assert_eq!((share.expanded, share.controls), (Some(true), Some(panel.id)));
    rt.key(0x1B);
    rt.frame(Duration::ZERO);

    rt.click("delete");
    rt.frame(Duration::ZERO);
    assert_eq!(labels(&rt), ["Delete this item?", "Cancel", "Delete"], "only the dialog");
    assert!(detail(&rt, "Delete this item?").modal);
    // Its backdrop takes the press, not what is under it.
    rt.pointer_down(30.0, 30.0);
    rt.pointer_up(30.0, 30.0);
    rt.frame(Duration::ZERO);
    assert!(!rt.state().confirm && !rt.state().share, "the backdrop dismissed it, Share never fired");
    settle(&mut rt);
    assert!(labels(&rt).iter().any(|l| l == "Share"));

    // A toast is a status item.
    rt.click("copy");
    rt.frame(Duration::ZERO);
    let toast = rt.list().hits.iter().find(|h| &*h.label == "Link copied").unwrap();
    assert_eq!(toast.role, stillkit::Role::Status);
}

#[test]
fn hovering_overlays_only_repaints() {
    let mut rt = demo();
    rt.right_click(100.0, 100.0);
    rt.frame(Duration::ZERO);
    settle(&mut rt);
    let runs = rt.view_runs();
    for name in ["cut", "copy-item", "paste", "cut"] {
        let at = rt.find(name).unwrap().center();
        rt.pointer_move(at.x, at.y);
        assert!(rt.frame(Duration::ZERO), "hover repaints");
    }
    // The rest of the window takes no hover while the menu is open.
    let share = rt.find("share").unwrap().center();
    rt.pointer_move(share.x, share.y);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.presented("share").unwrap().state, stillkit::interact::State::Rest);
    assert_eq!(rt.view_runs(), runs);
}

#[test]
fn overlays_squeezed_in_the_window_go_to_popup_surfaces() {
    let mut rt = Runtime::new(Demo::default(), view, theme(), (200, 120), 1.0).unwrap();
    rt.set_screen(Some(Rect::new(-300.0, -300.0, 1200.0, 900.0)));
    rt.click("share");
    rt.frame(Duration::ZERO);
    let (share, panel) = (rt.find("share").unwrap(), rt.find("share-panel").unwrap());
    assert_eq!(panel.top, share.bottom + 6.0, "placed against the screen, below its trigger");
    assert!(panel.right > 200.0 || panel.bottom > 120.0, "{panel:?} leaves the window");
    let [popup] = rt.popups() else { panic!("one popup: {:?}", rt.popups().len()) };
    assert!(popup.rect.left <= panel.left && popup.rect.right >= panel.right);
    let copy = popup.list.hits.iter().find(|h| &*h.label == "Copy link").unwrap();
    assert!(copy.rect.left >= 0.0 && copy.rect.right <= popup.rect.width(), "in its own pixels");
    // Input comes back in window pixels: the popup's hits are still the runtime's.
    rt.click("share-copy");
    rt.frame(Duration::ZERO);
    assert!(!rt.state().share && rt.state().toasts.len() == 1);
    settle(&mut rt);
    assert!(rt.popups().is_empty(), "gone once its exit ends");
    // Without a screen it stays in the window.
    rt.set_screen(None);
    rt.click("share");
    rt.frame(Duration::ZERO);
    assert!(rt.popups().is_empty());
    assert_eq!(rt.find("share-panel").unwrap().left, 8.0, "shifted inside the window");
}
