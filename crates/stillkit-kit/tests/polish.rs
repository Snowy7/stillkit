#![cfg(windows)]
#[allow(dead_code)]
#[path = "../examples/support/mod.rs"]
mod controls;
#[allow(dead_code)]
#[path = "../examples/kit_text.rs"]
mod editors;
#[allow(dead_code)]
#[path = "../examples/layout_support/mod.rs"]
mod layout;
use editors::overlays;
use std::time::Duration;
use stillkit::el::{Runtime, col, text};
use stillkit_kit::{Disclosure, Stepper};

#[test]
fn stepper_buttons_clamp_and_value_stays_centred_at_both_scales() {
    for (_, theme) in controls::themes() {
        for scale in [1, 2] {
            let control = theme.metrics.control * scale as f32;
            let mut rt = Runtime::new(
                0_f64,
                |v| {
                    Stepper::new("Copies", 0., 2., 1., *v)
                        .on_change(|s: &mut f64, v| *s = v)
                        .build()
                },
                theme.clone(),
                (300 * scale, 100 * scale),
                scale as f32,
            )
            .unwrap();
            rt.frame(Duration::ZERO);
            let minus = rt.find("Decrease").unwrap();
            let plus = rt.find("Increase").unwrap();
            assert_eq!(minus.height(), control);
            assert_eq!(minus.top, plus.top);
            let number = rt
                .tree()
                .into_iter()
                .find(|n| n.text.as_deref() == Some("0"))
                .unwrap();
            assert!(
                (number.rect.center().x - (minus.center().x + plus.center().x) / 2.).abs() <= 1.
            );
            rt.click("Decrease");
            rt.frame(Duration::ZERO);
            assert_eq!(*rt.state(), 0.);
            for expected in [1., 2., 2.] {
                rt.click("Increase");
                rt.frame(Duration::ZERO);
                assert_eq!(*rt.state(), expected);
            }
        }
    }
}

#[test]
fn disclosure_body_tracks_title_and_opening_keeps_the_title_stationary() {
    for (_, theme) in controls::themes() {
        for scale in [1, 2] {
            let mut rt = Runtime::new(
                true,
                |open| {
                    col().child(
                        Disclosure::new("Details", *open, text("Body").key("body"))
                            .on_toggle(|s: &mut bool| *s = !*s)
                            .build(),
                    )
                },
                theme.clone(),
                (400 * scale, 200 * scale),
                scale as f32,
            )
            .unwrap();
            rt.frame(Duration::ZERO);
            let title = rt
                .tree()
                .into_iter()
                .find(|n| n.text.as_deref() == Some("Details"))
                .unwrap()
                .rect;
            assert_eq!(title.left, rt.find("body").unwrap().left);
            rt.click("trigger");
            rt.frame(Duration::from_secs(2));
            let closed = rt
                .tree()
                .into_iter()
                .find(|n| n.text.as_deref() == Some("Details"))
                .unwrap()
                .rect;
            assert_eq!(title.left, closed.left);
        }
    }
}

/// Explicit audit export; normal tests retain the existing 1x golden gate.
#[test]
#[ignore = "writes full-resolution sheets to STILLKIT_AUDIT_DIR for visual inspection"]
fn export_scale_audit() {
    let out = std::path::PathBuf::from(
        std::env::var_os("STILLKIT_AUDIT_DIR").expect("set STILLKIT_AUDIT_DIR"),
    );
    std::fs::create_dir_all(&out).unwrap();
    for (name, theme) in controls::themes() {
        for scale in [1, 2] {
            for (family, image) in [
                (
                    "kit",
                    controls::sheet_at_scale(theme.clone(), scale).unwrap(),
                ),
                (
                    "layout",
                    layout::sheet_at_scale(theme.clone(), scale).unwrap(),
                ),
                (
                    "text",
                    editors::sheet_at_scale(theme.clone(), scale).unwrap(),
                ),
                (
                    "overlays",
                    overlays::sheet_at_scale(theme.clone(), scale).unwrap(),
                ),
            ] {
                stillkit::golden::write(&image, &out.join(format!("{family}-{name}@{scale}x.png")))
                    .unwrap();
            }
        }
    }
}
