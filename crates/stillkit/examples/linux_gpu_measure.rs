//! Linux measurement child: stdin `drag` runs ten seconds of scripted slider updates;
//! `quit` exits. It never requests activation or injects pointer input.
#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use std::{
        io::{BufRead, Write},
        time::{Duration, Instant},
    };
    use stillkit::platform::{Render, set_render};
    use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ui};
    set_render(Render::Cpu);
    let mode = std::env::args().nth(1).unwrap_or("cpu".into());
    let mut ui = Ui::new()?;
    let mut window = Surface::with_render(
        Kind::Popup { activate: false },
        "stillkit Linux measurement",
        Role::Window,
        if mode == "cpu" {
            Render::Cpu
        } else {
            Render::Gpu
        },
    )?;
    let mut other = if mode == "mixed" {
        Some(Surface::with_render(
            Kind::Popup { activate: false },
            "stillkit CPU beside Vulkan",
            Role::Window,
            Render::Cpu,
        )?)
    } else {
        None
    };
    let list = |value: f32| DisplayList {
        ops: vec![
            Op::Fill {
                rect: Rect::new(0., 0., 400., 220.),
                radius: 16.,
                color: Rgba::new(0.08, 0.1, 0.15, if mode == "glass" { 0.12 } else { 1. }),
            },
            Op::Text {
                rect: Rect::new(24., 24., 350., 35.),
                text: "CPU + Vulkan · Inter 600".into(),
                style: TextStyle {
                    weight: 600,
                    ..TextStyle::new("Inter", 20.)
                },
                color: Rgba::new(1., 1., 1., 1.),
            },
            Op::Fill {
                rect: Rect::new(24., 130., 352., 6.),
                radius: 3.,
                color: Rgba::new(0.4, 0.45, 0.5, 1.),
            },
            Op::Fill {
                rect: Rect::new(24., 130., 352. * value, 6.),
                radius: 3.,
                color: Rgba::new(0.2, 0.55, 1., 1.),
            },
            Op::Fill {
                rect: Rect::new(16. + 352. * value, 121., 24., 24.),
                radius: 12.,
                color: Rgba::new(1., 1., 1., 1.),
            },
        ],
        ..Default::default()
    };
    if mode == "glass" {
        window.set_glass_group(
            &[
                stillkit::Glass {
                    body: Rect::new(8., 8., 230., 204.),
                    radius: 22.,
                    material: stillkit::GlassMaterial {
                        clarity: 0.8,
                        blur: 8.,
                        refraction: 0.6,
                        highlight: 0.5,
                        tint: Rgba::TRANSPARENT,
                    },
                },
                stillkit::Glass {
                    body: Rect::new(248., 35., 144., 150.),
                    radius: 22.,
                    material: stillkit::GlassMaterial {
                        clarity: 0.8,
                        blur: 8.,
                        refraction: 0.6,
                        highlight: 0.5,
                        tint: Rgba::TRANSPARENT,
                    },
                },
            ],
            32.,
        );
    }
    window.present(&mut ui, (0, 0), (400, 220), list(0.5))?;
    if let Some(cpu) = &mut other {
        cpu.present(&mut ui, (0, 0), (400, 220), list(0.5))?;
    }
    assert!(!window.present(&mut ui, (0, 0), (400, 220), list(0.5))?);
    #[cfg(feature = "golden")]
    if let Some(path) = std::env::args().nth(2) {
        stillkit::golden::write(&window.image()?, std::path::Path::new(&path))?;
    }
    println!("ready");
    std::io::stdout().flush()?;
    let mut input = std::io::stdin().lock();
    loop {
        while stillkit::next_event().is_some() {}
        let mut fds = [
            libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stillkit::platform::linux::surface::connection_fd()?,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: both descriptors remain open; poll accesses two initialized entries.
        unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if fds[0].revents == 0 {
            continue;
        }
        let mut command = String::new();
        if input.read_line(&mut command)? == 0 || command.trim() == "quit" {
            break;
        }
        if command.trim() == "snap" {
            #[cfg(feature = "golden")]
            if let Some(path) = std::env::args().nth(2) {
                stillkit::golden::write(&window.image()?, std::path::Path::new(&path))?;
            }
            println!("snapshot");
            std::io::stdout().flush()?;
        }
        if command.trim() == "drag" {
            let start = Instant::now();
            let mut times = Vec::new();
            let mut frames = 0;
            while start.elapsed() < Duration::from_secs(10) {
                while stillkit::next_event().is_some() {}
                let before = Instant::now();
                let value = 0.5 + 0.45 * (start.elapsed().as_secs_f32() * 3.).sin();
                window.present(&mut ui, (0, 0), (400, 220), list(value))?;
                if let Some(cpu) = &mut other {
                    cpu.present(&mut ui, (0, 0), (400, 220), list(value))?;
                }
                times.push(before.elapsed().as_secs_f64() * 1000.);
                frames += 1;
                std::thread::sleep(
                    Duration::from_secs_f64(1. / 60.).saturating_sub(before.elapsed()),
                );
            }
            times.sort_by(f64::total_cmp);
            println!(
                "drag frames={frames} mean_ms={:.3} p95_ms={:.3}",
                times.iter().sum::<f64>() / times.len() as f64,
                times[times.len() * 95 / 100]
            );
            std::io::stdout().flush()?;
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn main() {}
