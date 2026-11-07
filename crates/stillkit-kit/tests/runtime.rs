#[cfg(windows)]
#[path = "../examples/support/mod.rs"]
mod support;
#[cfg(windows)]
mod native {
    use super::support;
    use std::time::Duration;
    use stillkit::el::Runtime;
    fn runtime(i: usize, disabled: bool) -> Runtime<support::State> {
        Runtime::new(
            support::State {
                value: 0.4,
                ..Default::default()
            },
            move |s| support::component(i, s, disabled),
            support::themes().remove(0).1,
            (320, 320),
            1.,
        )
        .unwrap()
    }
    #[test]
    fn activation_toggle_disabled_and_focus_order() {
        for i in [0, 1, 2, 3, 10, 12] {
            let mut r = runtime(i, false);
            assert!(r.click("target"));
            r.frame(Duration::ZERO);
            if [2, 3, 12].contains(&i) {
                assert!(r.state().on);
            } else {
                assert_eq!(r.state().presses, 1);
            }
            let mut r = runtime(i, true);
            r.click("target");
            r.frame(Duration::ZERO);
            assert!(!r.state().on);
            assert_eq!(r.state().presses, 0);
        }
        let mut r = runtime(0, false);
        r.tab(false);
        r.key(0x0d);
        r.frame(Duration::ZERO);
        assert_eq!(r.state().presses, 1);
        r.tab(false);
        r.key(0x20);
        r.frame(Duration::ZERO);
        assert_eq!(r.state().presses, 2);
        let mut r = runtime(0, true);
        r.tab(false);
        r.key(0x0d);
        r.frame(Duration::ZERO);
        assert_eq!(r.state().presses, 0);
    }
    #[test]
    fn selection_slider_quantization_and_disabled_drag() {
        for i in [4, 7] {
            let mut r = runtime(i, false);
            assert!(r.click("target/option-2"));
            r.frame(Duration::ZERO);
            assert_eq!(r.state().selected, 2);
            let checked = r
                .list()
                .hits
                .iter()
                .find(|h| h.label.as_ref() == "Maximum" || h.label.as_ref() == "Month")
                .unwrap();
            assert_eq!(checked.checked, Some(true));
        }
        for i in [5, 6] {
            let mut r = runtime(i, false);
            let at = r.find("target").unwrap();
            r.pointer_down(at.left + at.width() * 0.73, at.center().y);
            r.frame(Duration::ZERO);
            assert!((r.state().value - 0.7).abs() < 0.001);
            r.pointer_move(at.right + 30., at.center().y);
            r.frame(Duration::ZERO);
            assert_eq!(r.state().value, 1.);
            assert!(r.list().hits.iter().any(|h| h.label.ends_with("100%")));
            let mut r = runtime(i, true);
            r.click("target");
            r.frame(Duration::ZERO);
            assert_eq!(r.state().value, 0.4);
        }
    }
    #[test]
    fn hover_press_focus_repaint_without_rebuilding() {
        let mut r = runtime(0, false);
        let before = r.view_runs();
        let ops = r.list().ops.clone();
        let p = r.find("target").unwrap().center();
        r.pointer_move(p.x, p.y);
        r.frame(Duration::ZERO);
        assert_ne!(ops, r.list().ops);
        r.pointer_down(p.x, p.y);
        r.frame(Duration::ZERO);
        assert_eq!(r.view_runs(), before);
        r.pointer_up(p.x, p.y);
        r.frame(Duration::ZERO);
        assert_eq!(r.state().presses, 1);
    }
    #[test]
    fn calendar_leap_days_and_selection() {
        use stillkit_kit::CalendarMonth;
        for (year, has_29) in [(1900, false), (2000, true), (2024, true), (2026, false)] {
            let mut rt = Runtime::new(
                0u8,
                move |s| {
                    CalendarMonth::new(year, 2)
                        .key("cal")
                        .selected(*s)
                        .on_select(|s, d| *s = d)
                        .build()
                },
                support::themes().remove(0).1,
                (320, 320),
                1.,
            )
            .unwrap();
            assert_eq!(rt.find("cal/day-29").is_some(), has_29);
            assert!(rt.find("cal/day-30").is_none());
            rt.click("cal/day-14");
            rt.frame(Duration::ZERO);
            assert_eq!(*rt.state(), 14);
        }
    }
    #[test]
    fn rendered_text_contrast_on_its_own_surface() {
        use stillkit::Op;
        use stillkit::theme::{contrast, over};
        for (name, t) in support::themes() {
            for i in 0..18 {
                let r = Runtime::new(
                    support::State {
                        value: 0.45,
                        ..Default::default()
                    },
                    move |s| support::component(i, s, false),
                    t.clone(),
                    (340, 360),
                    1.,
                )
                .unwrap();
                let ops = &r.list().ops;
                for (index, op) in ops.iter().enumerate() {
                    if let Op::Text {
                        rect, text, color, ..
                    } = op
                    {
                        // Font icon glyphs are symbols, not text labels.
                        if text
                            .chars()
                            .any(|c| (0xe000..=0xf8ff).contains(&(c as u32)))
                        {
                            continue;
                        }
                        let bg =
                            ops[..index]
                                .iter()
                                .fold(t.color.surface.with_alpha(1.), |bg, op| match op {
                                    Op::Fill {
                                        rect: fill, color, ..
                                    } if fill.contains(rect.center()) => over(*color, bg),
                                    _ => bg,
                                });
                        let secondary = [
                            "Connected",
                            "Connections",
                            "Concentric corners and theme padding",
                            "Add a folder to get started",
                            "Search your library",
                            "M",
                            "T",
                            "W",
                            "F",
                            "S",
                        ]
                        .contains(&text.as_ref());
                        let floor = if secondary { 4.5 } else { 7. };
                        assert!(
                            contrast(*color, bg) >= floor,
                            "{name} component {i} {text}: {} < {floor} over {bg:?}, ink {color:?}",
                            contrast(*color, bg)
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn focus_skips_disabled_and_reverses_in_document_order() {
        use stillkit::el::col;
        use stillkit_kit::Button;
        let mut rt = Runtime::new(
            Vec::<&str>::new(),
            |_| {
                col()
                    .child(
                        Button::new("First")
                            .key("first")
                            .on_press(|s: &mut Vec<&str>| s.push("first"))
                            .build(),
                    )
                    .child(
                        Button::new("Skipped")
                            .key("skip")
                            .disabled(true)
                            .on_press(|s: &mut Vec<&str>| s.push("skip"))
                            .build(),
                    )
                    .child(
                        Button::new("Last")
                            .key("last")
                            .on_press(|s: &mut Vec<&str>| s.push("last"))
                            .build(),
                    )
            },
            support::themes().remove(0).1,
            (320, 240),
            1.,
        )
        .unwrap();
        for back in [false, false, true] {
            rt.tab(back);
            rt.key(0x0d);
            rt.frame(Duration::ZERO);
        }
        assert_eq!(rt.state(), &["first", "last", "first"]);
    }
    #[test]
    fn gallery_goldens() {
        for (name, t) in support::themes() {
            let image = support::sheet(t).unwrap();
            stillkit::golden::check(
                &image,
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../docs/media/kit/kit-{name}.png")),
            )
            .unwrap();
        }
    }
}
#[test]
fn component_surfaces_meet_contrast() {
    use stillkit::theme::{SYSTEM_BLUE, contrast};
    use stillkit::{Rgba, Scheme, Theme};
    use stillkit_kit::{button_surface, readable, surface};
    for scheme in [Scheme::Dark, Scheme::Light] {
        for t in [
            Theme::liquid(scheme, SYSTEM_BLUE),
            Theme::one_ui(scheme, SYSTEM_BLUE),
            Theme::solid(scheme, SYSTEM_BLUE),
            Theme::fluent(scheme, SYSTEM_BLUE),
            Theme::mono(scheme, SYSTEM_BLUE),
        ] {
            for fill in [
                Rgba::TRANSPARENT,
                t.color.card,
                t.color.fill,
                t.color.well,
                t.color.elevated,
                t.color.accent_wash,
                button_surface(&t),
            ] {
                let bg = surface(&t, fill);
                for (ink, floor) in [
                    (t.color.label, 7.),
                    (t.color.secondary, 4.5),
                    (t.color.accent, 3.),
                ] {
                    let actual = readable(ink, bg, floor);
                    assert!(
                        contrast(actual, bg) >= floor,
                        "{scheme:?} {floor}: {}",
                        contrast(actual, bg)
                    );
                }
            }
        }
    }
}
