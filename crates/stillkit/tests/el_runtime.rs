//! The element runtime, headless, on the hello example: handlers, the keyed diff, hover and
//! focus without re-running the view, damage, and the agent tooling (snap, tree, script).

#[allow(dead_code)]
#[path = "../examples/el_hello.rs"]
mod hello;

use std::time::Duration;

use hello::{Hello, theme, view};
use stillkit::el::{Runtime, Script};
use stillkit::interact::State;
use stillkit::{Role, Rgba};

const FRAME: Duration = Duration::from_millis(16);

fn runtime(scale: f32) -> Runtime<Hello> {
    let size = ((320.0 * scale) as u32, (180.0 * scale) as u32);
    Runtime::new(Hello { on: false }, view, theme(), size, scale).unwrap()
}

#[test]
fn a_click_runs_the_handler_between_frames_and_the_view_once() {
    let mut rt = runtime(1.0);
    assert_eq!(rt.view_runs(), 1);
    assert!(rt.click("switch"));
    assert!(!rt.state().on, "handlers wait for the frame");
    assert!(rt.frame(FRAME));
    assert!(rt.state().on);
    assert_eq!(rt.view_runs(), 2);
    let label = rt.presented("label").unwrap();
    assert_eq!(label.text.as_deref(), Some("The light is on"));
    let switch = rt.presented("switch").unwrap();
    assert_eq!((switch.role, switch.checked), (Some(Role::Switch), Some(true)));
    assert_eq!(switch.fill, Some(rt.theme().color.accent));
}

#[test]
fn nothing_changes_nothing_runs_and_nothing_is_painted() {
    let mut rt = runtime(1.0);
    let (views, paints) = (rt.view_runs(), rt.paints());
    for _ in 0..10 {
        assert!(!rt.frame(FRAME));
    }
    rt.pointer_move(5.0, 5.0); // over nothing interactive
    assert!(!rt.frame(FRAME));
    assert_eq!((rt.view_runs(), rt.paints()), (views, paints));
}

#[test]
fn hover_and_press_repaint_one_control_without_running_the_view() {
    let mut rt = runtime(1.0);
    let switch = rt.find("switch").unwrap();
    let at = switch.center();
    rt.pointer_move(at.x, at.y);
    assert!(rt.frame(FRAME));
    assert_eq!(rt.view_runs(), 1, "hover never runs the view");
    assert_eq!(rt.presented("switch").unwrap().state, State::Hover);
    let damage = rt.damage().expect("the wash changed");
    assert!(
        damage.left >= switch.left - 1.0 && damage.right <= switch.right + 1.0,
        "only the switch's wash is redrawn: {damage:?} in {switch:?}"
    );
    rt.pointer_down(at.x, at.y);
    rt.frame(FRAME);
    assert_eq!(rt.presented("switch").unwrap().state, State::Pressed);
    assert_eq!(rt.view_runs(), 1);
    rt.pointer_up(at.x, at.y);
    rt.frame(FRAME);
    assert_eq!(rt.view_runs(), 2, "the release fired the handler");
}

#[test]
fn the_keyed_diff_reports_what_moved_and_restyles_only_that() {
    let mut rt = runtime(1.0);
    let before = rt.find("knob").unwrap();
    rt.update(|s| s.on = true);
    rt.frame(FRAME);
    let knob = rt
        .changes()
        .iter()
        .find(|c| &*c.key == "knob")
        .expect("the knob moved");
    assert_eq!(knob.from.unwrap().rect, before);
    assert_eq!(knob.to.unwrap().rect, before.offset(16.0, 0.0));
    let switch = rt.changes().iter().find(|c| &*c.key == "switch").unwrap();
    assert_ne!(switch.from.unwrap().fill, switch.to.unwrap().fill);
    // Layout sees only the label's text and the knob's inset (a fill is paint, not layout).
    assert_eq!(rt.touched(), 2);
}

#[test]
fn tab_focuses_the_switch_and_enter_presses_it() {
    let mut rt = runtime(1.0);
    rt.tab(false);
    assert!(rt.frame(FRAME), "the focus ring shows");
    assert!(rt.presented("switch").unwrap().focused);
    assert_eq!(rt.view_runs(), 1);
    rt.key(0x0D);
    rt.frame(FRAME);
    assert!(rt.state().on);
    rt.key(0x20);
    rt.frame(FRAME);
    assert!(!rt.state().on, "Space presses too");
}

#[test]
fn a_snapshot_renders_at_the_requested_scale() {
    let mut rt = runtime(2.0);
    rt.update_instant(|s| s.on = true); // the settled frame: the knob springs otherwise
    rt.frame(FRAME);
    let image = rt.snap().unwrap();
    assert_eq!((image.width, image.height), (640, 360));
    // Inside the track, left of the knob: the accent, premultiplied BGRA.
    let track = rt.find("switch").unwrap();
    let [b, g, r, a] = image.pixel((track.left + 8.0) as u32, track.center().y as u32);
    let Rgba { r: ar, g: ag, b: ab, .. } = rt.theme().color.accent;
    let near = |px: u8, c: f32| (f32::from(px) - c * 255.0).abs() < 8.0;
    assert!(a == 255 && near(r, ar) && near(g, ag) && near(b, ab), "{:?}", [b, g, r, a]);
}

#[test]
fn a_script_replays_on_the_virtual_clock_and_the_tree_dumps_it() {
    let mut rt = runtime(2.0);
    // Scripts are in design pixels; `find` answers in device pixels.
    let at = rt.find("switch").unwrap().center();
    let (x, y) = (at.x / 2.0, at.y / 2.0);
    let script = Script::parse(&format!(
        r#"[{{"at": 0.5, "move": [{x}, {y}]}}, {{"at": 0.6, "down": [{x}, {y}]}},
            {{"at": 0.7, "up": [{x}, {y}]}}, {{"at": 2.0, "click": "switch"}}]"#
    ))
    .unwrap();
    rt.play(&script, Some(Duration::from_secs(1)));
    assert_eq!(rt.now(), Duration::from_secs(1));
    assert!(rt.state().on, "the steps before 1 s ran; the click at 2 s did not");
    let tree = rt.tree_json();
    assert!(tree.starts_with("{\"scale\":2,\"size\":[320,180]"), "{tree}");
    assert!(tree.contains(r#""key":"switch","path":"/switch","role":"Switch","label":"Light""#), "{tree}");
    assert!(tree.contains(r#""text":"The light is on""#), "{tree}");
    assert!(tree.contains(r#""key":"knob","path":"/switch/knob""#), "{tree}");
}

#[test]
fn a_label_with_a_role_is_announced_enabled_but_takes_no_focus_or_hover() {
    use stillkit::el::{El, col, el, text};
    fn view(_: &()) -> El<()> {
        col()
            .child(text("Sound").role(Role::Heading).key("title"))
            .child(el().key("go").size(20.0).label("Go").on_press(|_: &mut ()| {}))
    }
    let mut rt = Runtime::new((), view, theme(), (200, 100), 1.0).unwrap();
    let title = rt.list().hits.iter().find(|h| &*h.label == "Sound").cloned().unwrap();
    assert!(title.enabled && !title.actionable, "{title:?}");
    let at = rt.find("title").unwrap().center();
    rt.pointer_move(at.x, at.y);
    assert!(!rt.frame(FRAME), "no hover on a label");
    rt.tab(false);
    rt.frame(FRAME);
    assert!(rt.presented("go").unwrap().focused, "Tab lands on the button");
    rt.tab(false);
    rt.frame(FRAME);
    assert!(rt.presented("go").unwrap().focused, "and only on it");
}
