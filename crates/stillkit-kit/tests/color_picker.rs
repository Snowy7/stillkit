//! The colour picker headless: dragging previews every move and commits once on release.
use std::time::Duration;

use stillkit::el::Runtime;
use stillkit::{Rgba, Scheme, Style, Theme};
use stillkit_kit::{ColorPicker, Hsva};

#[derive(Default)]
struct State {
    color: Option<Hsva>,
    picks: Vec<bool>,
}

fn view(s: &State) -> stillkit::el::El<State> {
    let color = s.color.unwrap_or(Hsva::from_rgba(Rgba::new(0.8, 0.1, 0.1, 1.)));
    stillkit::el::col().pad(10.).child(
        ColorPicker::new(color)
            .width(200.)
            .on_change(|s: &mut State, c, done| {
                s.color = Some(c);
                s.picks.push(done);
            })
            .build()
            .key("picker"),
    )
}

#[test]
fn dragging_the_square_previews_then_commits() {
    let theme = Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
    let mut rt = Runtime::new(State::default(), view, theme, (260, 260), 1.).unwrap();
    rt.frame(Duration::ZERO);
    let r = rt.find("picker").unwrap();
    // The square is the picker's first 150 px: its top-left is white, its bottom black.
    rt.pointer_down(r.left + 20., r.top + 20.);
    rt.pointer_move(r.left + 180., r.top + 10.);
    rt.frame(Duration::from_millis(16));
    let c = rt.state().color.unwrap();
    assert!(c.saturation > 0.85 && c.value > 0.9, "top right is saturated and bright: {c:?}");
    assert!((c.hue - Hsva::from_rgba(Rgba::new(0.8, 0.1, 0.1, 1.)).hue).abs() < 0.01, "the hue holds");
    rt.pointer_up(r.left + 180., r.top + 10.);
    rt.frame(Duration::from_millis(16));
    let picks = &rt.state().picks;
    assert!(picks.len() >= 3, "{picks:?}");
    assert!(picks[..picks.len() - 1].iter().all(|done| !done), "moves preview");
    assert_eq!(picks.last(), Some(&true), "the release commits");
}
