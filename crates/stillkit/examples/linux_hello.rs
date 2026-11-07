//! A CPU-drawn Wayland window. Close it normally; no idle timer is installed.
#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use stillkit::{DisplayList, Event, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ui};
    let mut ui = Ui::new()?;
    let mut window = Surface::new(
        Kind::App { min: (320, 160) },
        "stillkit · Linux CPU",
        Role::Window,
    )?;
    let mut scale = window.scale() as f32;
    let mut size = ((400. * scale) as u32, (220. * scale) as u32);
    window.set_chrome(&[(
        Rect::new(0., 0., 400. * scale, 28. * scale),
        stillkit::Chrome::Caption,
    )]);
    let animate = std::env::args().any(|a| a == "--animate");
    let _frames = animate.then(|| window.animation_frames(std::time::Duration::from_millis(600)));
    let mut frame = 0u32;
    loop {
        let list = DisplayList {
            ops: vec![
                Op::Fill {
                    rect: Rect::new(0., 0., size.0 as f32, size.1 as f32),
                    radius: 12. * scale,
                    color: Rgba::rgb8([25, 31, 46], 1.),
                },
                Op::Text {
                    rect: Rect::new(24. * scale, 32. * scale, 350. * scale, 40. * scale),
                    text: "Hello from stillkit".into(),
                    style: TextStyle {
                        weight: 600,
                        ..TextStyle::new("Inter", 22. * scale)
                    },
                    color: Rgba::rgb8([235, 240, 249], 1.),
                },
                Op::Text {
                    rect: Rect::new(24. * scale, 86. * scale, 350. * scale, 30. * scale),
                    text: "Wayland · CPU only · idle means idle".into(),
                    style: TextStyle::new("Inter", 14. * scale),
                    color: Rgba::rgb8([180, 192, 214], 1.),
                },
            ],
            ..Default::default()
        };
        let mut list = list;
        if animate {
            list.ops.push(Op::Arc {
                center: stillkit::Point::new(48. * scale, 154. * scale),
                radius: 16. * scale,
                sweep: (frame as f32 / 36.).min(1.) * std::f32::consts::TAU,
                width: 3. * scale,
                color: Rgba::rgb8([139, 184, 255], 1.),
            });
        }
        #[cfg(feature = "golden")]
        if frame == 0
            && let Some(path) =
                std::env::args().find_map(|a| a.strip_prefix("--snapshot=").map(str::to_owned))
        {
            stillkit::golden::write(
                &stillkit::render(&mut ui, &list, size.0, size.1)?,
                std::path::Path::new(&path),
            )?;
        }
        window.present(&mut ui, (0, 0), size, list)?;
        match stillkit::next_event() {
            Some((id, event)) if id == window.id() => match event {
                Event::Close => break,
                Event::Frame => frame += 1,
                Event::Resized(w, h) => size = (w, h),
                Event::DpiChanged(dpi) => {
                    let next = dpi as f32 / 96.;
                    size = (
                        (size.0 as f32 / scale * next) as u32,
                        (size.1 as f32 / scale * next) as u32,
                    );
                    scale = next;
                }
                Event::Press(p, stillkit::Button::Left) if p.at.y < 28. * scale => {
                    window.start_move()?
                }
                _ => {}
            },
            None => break,
            _ => {}
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn main() {}
