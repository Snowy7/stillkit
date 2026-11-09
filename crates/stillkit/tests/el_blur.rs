//! Retained filter radius, damage dependencies and CPU pixels.
use std::{sync::Arc, time::Duration};
use stillkit::el::{Runtime, el};
use stillkit::{DisplayList, Op, Rect, Rgba, Scheme, Theme};
fn fill(x: f32, color: Rgba) -> Op {
    Op::Fill {
        rect: Rect::new(x, 0.0, 40.0, 80.0),
        radius: 0.0,
        color,
    }
}
fn scene(backdrop: bool, sigma: f32) -> DisplayList {
    let source = vec![
        fill(0.0, Rgba::new(1.0, 0.0, 0.0, 1.0)),
        fill(40.0, Rgba::new(0.0, 0.0, 1.0, 1.0)),
    ];
    DisplayList {
        ops: if backdrop {
            let mut ops = source;
            ops.push(Op::BackdropBlur {
                rect: Rect::new(10.0, 10.0, 60.0, 60.0),
                radius: 12.0,
                sigma,
                opacity: 1.0,
            });
            ops
        } else {
            vec![Op::Blurred {
                sigma,
                ops: source.into(),
            }]
        },
        ..Default::default()
    }
}
#[test]
fn radii_spring_and_reduced_motion_snaps() {
    for reduced in [false, true] {
        let mut rt = Runtime::new(
            false,
            |on| {
                el().size(80.0)
                    .blur(if *on { 8.0 } else { 2.0 })
                    .backdrop_blur(if *on { 6.0 } else { 1.0 })
                    .animate()
                    .child(el().size(30.0).fill(Rgba::new(1.0, 0.0, 0.0, 1.0)))
            },
            Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
            (100, 100),
            1.0,
        )
        .unwrap();
        rt.set_reduced_motion(reduced);
        rt.update(|on| *on = true);
        rt.frame(Duration::ZERO);
        rt.frame(Duration::from_millis(16));
        let Op::Blurred { sigma, ops } = &rt.list().ops[0] else {
            panic!("subtree scope")
        };
        if reduced {
            assert_eq!(*sigma, 8.0)
        } else {
            assert!(*sigma > 2.0 && *sigma < 8.0)
        }
        assert!(matches!(ops[0], Op::BackdropBlur { .. }));
        for _ in 0..300 {
            rt.frame(Duration::from_millis(16));
        }
        assert!(!rt.animating());
        assert!(!rt.frame(Duration::from_secs(1)));
    }
}
#[test]
fn filters_move_scale_and_damage_their_dependencies() {
    let before = scene(true, 4.0);
    let mut after = before.clone();
    after.ops[0] = fill(0.0, Rgba::new(0.0, 1.0, 0.0, 1.0));
    let damage = after.damage(&before).unwrap();
    assert!(damage.right >= 52.0 && damage.right < 70.0);
    assert!(before.damage(&before).is_none());
    let mut op = scene(false, 4.0).ops.remove(0);
    op.translate(5.0, 6.0);
    op.scale(2.0);
    let Op::Blurred { sigma, ops } = op else {
        unreachable!()
    };
    assert_eq!(sigma, 8.0);
    let Op::Fill { rect, .. } = ops[0] else {
        unreachable!()
    };
    assert_eq!(rect, Rect::new(10.0, 12.0, 80.0, 160.0));
}
#[test]
fn cpu_filters_mix_content_and_keep_backdrop_outside_mask() {
    let mut ui = stillkit::Ui::new().unwrap();
    for backdrop in [false, true] {
        let list = scene(backdrop, 4.0);
        let image = stillkit::render(&mut ui, &list, 100, 100).unwrap();
        let middle = image.pixel(40, 40);
        assert!(middle[0] > 0 && middle[2] > 0, "{middle:?}");
        if backdrop {
            assert_eq!(image.pixel(4, 40), [0, 0, 255, 255]);
        }
        #[cfg(all(windows, feature = "golden"))]
        stillkit::golden::check(
            &image,
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(if backdrop {
                "tests/golden/blur-backdrop.png"
            } else {
                "tests/golden/blur-subtree.png"
            }),
        )
        .unwrap();
    }
}
#[test]
#[ignore = "release CPU filter benchmark; intentionally defeats the frame cache"]
fn cpu_blur_cost() {
    let mut ui = stillkit::Ui::new().unwrap();
    for (w, h) in [(100, 100), (320, 180), (800, 600)] {
        for backdrop in [false, true] {
            let mut samples = Vec::new();
            for i in 0..120 {
                let source: Arc<[Op]> = vec![Op::Fill {
                    rect: Rect::new(0.0, 0.0, w as f32 / 2.0, h as f32),
                    radius: 0.0,
                    color: Rgba::new(i as f32 / 120.0, 0.2, 0.8, 1.0),
                }]
                .into();
                let ops = if backdrop {
                    let mut ops = source.to_vec();
                    ops.push(Op::BackdropBlur {
                        rect: Rect::new(0.0, 0.0, w as f32, h as f32),
                        radius: 0.0,
                        sigma: 8.0,
                        opacity: 1.0,
                    });
                    ops
                } else {
                    vec![Op::Blurred {
                        sigma: 8.0,
                        ops: source,
                    }]
                };
                let start = std::time::Instant::now();
                let _ = stillkit::render(
                    &mut ui,
                    &DisplayList {
                        ops,
                        ..Default::default()
                    },
                    w,
                    h,
                )
                .unwrap();
                if i >= 20 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{w}x{h} backdrop={backdrop}: median {:.3} ms p95 {:.3} ms",
                samples[50], samples[94]
            );
        }
    }
}

#[test]
fn partial_cpu_updates_equal_fresh_filters() {
    use stillkit::platform::Renderer;
    let mut ui = stillkit::Ui::new().unwrap();
    for backdrop in [false, true] {
        let canvas = stillkit::platform::current::Renderer::new(&ui, 100, 100).unwrap();
        let before = scene(backdrop, 4.0);
        canvas
            .draw(&mut ui.text, &before, stillkit::PixelRect::full(100, 100))
            .unwrap();
        let mut after = scene(backdrop, 4.0);
        if backdrop {
            after.ops[0] = fill(0.0, Rgba::new(0.0, 1.0, 0.0, 1.0));
        } else if let Op::Blurred { ops, .. } = &mut after.ops[0] {
            Arc::make_mut(ops)[0] = fill(0.0, Rgba::new(0.0, 1.0, 0.0, 1.0));
        }
        // Arbitrary narrow damage exercises the kernel's halo independently of scene damage.
        let area = stillkit::PixelRect {
            x: 36,
            y: 34,
            width: 8,
            height: 12,
        };
        canvas.draw(&mut ui.text, &after, area).unwrap();
        let fresh = stillkit::render(&mut ui, &after, 100, 100).unwrap();
        canvas
            .read(area, |y, row| {
                let i = ((area.y + y) * 100 + area.x) as usize * 4;
                assert_eq!(row, &fresh.pixels[i..i + row.len()]);
            })
            .unwrap();
    }
}

#[cfg(all(windows, feature = "gpu"))]
#[test]
fn native_gpu_filters_mix_pixels_inside_the_mask() {
    let mut ui = stillkit::Ui::new().unwrap();
    for backdrop in [false, true] {
        let Some(image) =
            stillkit::platform::windows::gpu::render(&mut ui, &scene(backdrop, 4.0), 100, 100)
                .unwrap()
        else {
            return;
        };
        let p = image.pixel(40, 40);
        assert!(p[0] > 20 && p[2] > 20, "{p:?}");
        if backdrop {
            assert_eq!(image.pixel(4, 40), [0, 0, 255, 255]);
        }
    }
}

#[test]
fn a_uniform_translucent_backdrop_keeps_its_alpha() {
    let mut ui=stillkit::Ui::new().unwrap();
    let list=DisplayList {ops:vec![Op::Fill{rect:Rect::new(0.0,0.0,100.0,100.0),radius:0.0,color:Rgba::new(0.2,0.4,0.6,0.5)},Op::BackdropBlur{rect:Rect::new(20.0,20.0,60.0,60.0),radius:12.0,sigma:2.0,opacity:0.5}],..Default::default()};
    let image=stillkit::render(&mut ui,&list,100,100).unwrap();
    assert_eq!(image.pixel(50,50),image.pixel(10,50));
    assert_eq!(image.pixel(20,20),image.pixel(10,50));
}
