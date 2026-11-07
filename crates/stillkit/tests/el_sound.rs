//! Phase 3's gate on the "Sound" panel: the element tree damages exactly what the same panel
//! placed by hand with `Frame` does, so surfaces redraw no more than before.

#[allow(dead_code)]
#[path = "../examples/el_sound.rs"]
mod sound;

use std::time::Duration;

use sound::{Sound, theme, view};
use stillkit::el::Runtime;
use stillkit::icons::Icon;
use stillkit::layout::Insets;
use stillkit::{DisplayList, Frame, Interaction, Rect, TextRole, Theme, Ui};

const SIZE: (u32, u32) = (340, 160);

/// The plan's "today" version, placed with rect slicing.
fn by_hand(ui: &mut Ui, t: &Theme, s: f32, m: &Sound) -> DisplayList {
    const VOLUME: u32 = 1;
    const MUTE: u32 = 2;
    let ix = Interaction::default();
    let mut f = Frame::new(&mut ui.text, t, s, &ix);
    let full = Rect::new(0.0, 0.0, SIZE.0 as f32 * s, SIZE.1 as f32 * s);
    let body = full.inset(Insets::all(f.px(t.metrics.pad)));
    let (title, rest) = body.take_top(f.line(TextRole::Title2), f.px(8.0));
    f.label(title.left, title.top, "Sound", TextRole::Title2, t.color.label, title.width());
    let (row, rest) = rest.take_top(f.px(t.metrics.control), f.px(8.0));
    f.slider(VOLUME, row, m.level, "Volume", Some(Icon::Volume(2)));
    let (row, _) = rest.take_top(f.px(t.metrics.item), 0.0);
    f.switch(MUTE, row.right, row.center().y, m.muted, "Mute", true);
    f.list
}

#[test]
fn a_slider_change_damages_the_same_rect_by_hand_and_through_el() {
    for s in [1.0, 1.5, 2.0] {
        let theme = theme();
        let mut ui = Ui::new().unwrap();
        let state = |level| Sound { level, muted: false };
        let before = by_hand(&mut ui, &theme, s, &state(0.4));
        let after = by_hand(&mut ui, &theme, s, &state(0.65));
        let by_hand = after.damage(&before).expect("the fill moved");

        let size = ((SIZE.0 as f32 * s) as u32, (SIZE.1 as f32 * s) as u32);
        let mut rt = Runtime::new(state(0.4), view, theme.clone(), size, s).unwrap();
        rt.update(|m| m.level = 0.65);
        assert!(rt.frame(Duration::from_millis(16)));
        assert_eq!(rt.damage(), Some(by_hand), "at {s}x");
    }
}

#[test]
fn dragging_the_slider_sets_the_level_and_mute_toggles() {
    let mut rt = Runtime::new(Sound { level: 0.4, muted: false }, view, theme(), SIZE, 1.0).unwrap();
    let volume = rt.find("volume").unwrap();
    let y = volume.center().y;
    rt.pointer_down(volume.left + volume.width() * 0.25, y);
    rt.pointer_move(volume.left + volume.width() * 0.5, y);
    rt.pointer_move(volume.left + volume.width() * 0.9, y);
    rt.frame(Duration::ZERO);
    assert!((rt.state().level - 0.9).abs() < 0.01, "the latest place wins: {}", rt.state().level);
    assert_eq!(rt.view_runs(), 2, "one handler run and one view per frame");
    rt.pointer_up(volume.left + volume.width() * 0.9, y);
    // The kit switch's own hit (and wash) answers to the element's id.
    assert!(rt.click("mute"));
    rt.frame(Duration::ZERO);
    assert!(rt.state().muted);
    let hits = &rt.list().hits;
    assert!(hits.iter().any(|h| &*h.label == "Volume 90%"));
    assert!(hits.iter().any(|h| &*h.label == "Mute" && h.checked == Some(true)));
}
