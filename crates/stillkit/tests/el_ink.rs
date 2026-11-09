//! Ink inheritance, headless: inside a `cascade_ink` element, text and icons without their own
//! colour take its ink and follow its hover look; their own colour wins; without the opt-in
//! they keep the theme's label ink.

use std::time::Duration;

use stillkit::el::{El, Runtime, col, row, text};
use stillkit::{Op, Rgba, Scheme, Style, Theme};

const REST: Rgba = Rgba::new(0.5, 0.5, 0.5, 1.0);
const HOT: Rgba = Rgba::new(1.0, 0.2, 0.2, 1.0);
const OWN: Rgba = Rgba::new(0.2, 1.0, 0.2, 1.0);

fn view(_: &()) -> El<()> {
    col().pad(10.0).gap(10.0)
        .child(row().key("button").pad(6.0).color(REST).cascade_ink().on_press(|_| {}).hovered(|e| e.color(HOT))
            .child(text("inherits")).child(text("own").color(OWN)))
        .child(row().key("plain").pad(6.0).color(REST).child(text("label")))
}

fn ink(rt: &Runtime<()>, s: &str) -> Rgba {
    rt.list().ops.iter().find_map(|op| match op {
        Op::Text { text, color, .. } if &**text == s => Some(*color),
        _ => None,
    }).unwrap()
}

#[test]
fn inherited_ink_follows_the_parents_hover_look() {
    let theme = Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
    let label = theme.color.label;
    let mut rt = Runtime::new((), view, theme, (300, 200), 1.0).unwrap();
    rt.frame(Duration::ZERO);
    assert_eq!(ink(&rt, "inherits"), REST);
    assert_eq!(ink(&rt, "own"), OWN);
    assert_eq!(ink(&rt, "label"), label, "no opt-in: the theme's label");
    let b = rt.find("button").unwrap();
    rt.pointer_move(b.center().x, b.center().y);
    for _ in 0..60 {
        rt.frame(Duration::from_millis(16));
    }
    assert_eq!(ink(&rt, "inherits"), HOT, "follows the hover look");
    assert_eq!(ink(&rt, "own"), OWN);
}
