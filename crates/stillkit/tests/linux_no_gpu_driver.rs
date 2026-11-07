#![cfg(target_os = "linux")]
use stillkit::{DisplayList, Op, Rect, Rgba, TextStyle, Ui, modules, render};
#[test]
fn cpu_shapes_and_native_glyphs_load_no_gpu_libraries() {
    let mut ui = Ui::new().unwrap();
    let list = DisplayList {
        ops: vec![
            Op::Fill {
                rect: Rect::new(0., 0., 200., 50.),
                radius: 8.,
                color: Rgba::new(0.1, 0.2, 0.3, 0.9),
            },
            Op::Text {
                rect: Rect::new(10., 10., 180., 30.),
                text: "CPU semibold".into(),
                style: TextStyle {
                    weight: 600,
                    ..TextStyle::new("Inter", 14.)
                },
                color: Rgba::new(1., 1., 1., 1.),
            },
        ],
        ..Default::default()
    };
    let image = render(&mut ui, &list, 200, 50).unwrap();
    assert!(image.pixels.iter().any(|b| *b != 0));
    let drivers: Vec<_> = modules::loaded()
        .into_iter()
        .filter(|m| modules::is_gpu_driver(m))
        .collect();
    assert!(drivers.is_empty(), "GPU libraries: {drivers:?}");
}
