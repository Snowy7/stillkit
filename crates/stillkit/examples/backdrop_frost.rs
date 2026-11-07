//! A One UI panel and a Fluent flyout over the desktop, in CPU mode with the system backdrop
//! (docs/ui.md, "System backdrop"): DWM blurs what is behind them, and nothing in this process
//! touches the GPU. No backdrop is installed, so the built-in one draws the kit's glass.
//!
//! ```text
//! cargo run --release -p stillkit --example backdrop_frost                 # 60 s, or Esc
//! cargo run --release -p stillkit --example backdrop_frost -- --shot <png>  # see below
//! cargo run --release -p stillkit --example backdrop_frost -- --fill        # the flat fill, to compare
//! ```
//!
//! `--shot` puts an opaque striped window of its own under the panels, captures only the
//! panels' rectangle (so nothing but this process's windows is in the picture), prints the
//! modules loaded, and exits. The windows never take the focus.

#[cfg(not(windows))]
fn main() {
    eprintln!("The system backdrop needs Windows.");
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    native::main()
}

#[cfg(windows)]
mod native {
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result};
    use stillkit::icons::Icon;
    use stillkit::interact::Interaction;
    use stillkit::kit::{Accessory, Frame, Lead, Row, Tile};
    use stillkit::layout::{Align, Insets, Stack, grid};
    use stillkit::{
        DisplayList, Event, Kind, Op, Rect, Rgba, Role, Scheme, Style, Surface, TextRole, Theme,
        Ui, modules, next_event,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, TranslateMessage,
    };

    const PANEL: (i32, i32, u32, u32) = (300, 200, 360, 420);
    const FLYOUT: (i32, i32, u32, u32) = (690, 200, 300, 236);

    pub fn main() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let shot = args.iter().position(|a| a == "--shot").and_then(|i| args.get(i + 1)).cloned();
        // `--fill`: the looks as they were without a backdrop (a flat tinted fill), to compare.
        let fill = args.iter().any(|a| a == "--fill");
        let mut ui = Ui::new()?;
        let stripes = match shot {
            Some(_) => {
                let mut s = Surface::new(Kind::Popup { activate: false }, "stripes", Role::Pane)?;
                let (x, y, w, h) = (PANEL.0 - 60, PANEL.1 - 60, (FLYOUT.0 + FLYOUT.2 as i32 + 60 - (PANEL.0 - 60)) as u32, PANEL.3 + 120);
                s.present(&mut ui, (x, y), (w, h), stripes(w, h))?;
                s.show();
                Some(s)
            }
            None => None,
        };
        let one_ui = Theme::new(Style::OneUi, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
        let fluent = Theme::new(Style::Fluent, Scheme::Light, stillkit::theme::SYSTEM_BLUE);
        let mut panel = Surface::new(Kind::Popup { activate: false }, "One UI panel", Role::Dialog)?;
        let mut flyout = Surface::new(Kind::Popup { activate: false }, "Fluent flyout", Role::Menu)?;
        for (surface, theme, at, view) in [
            (&mut panel, &one_ui, PANEL, control_panel as fn(&mut Frame, &Theme, Rect)),
            (&mut flyout, &fluent, FLYOUT, menu),
        ] {
            let (list, glass) = draw(&mut ui, surface, theme, (at.2, at.3), view, fill);
            surface.present(&mut ui, (at.0, at.1), (at.2, at.3), list)?;
            surface.show();
            surface.set_glass(glass);
        }
        println!(
            "glass: panel {}, flyout {}; render {:?}",
            panel.has_glass(),
            flyout.has_glass(),
            panel.render()
        );
        if let Some(path) = shot {
            // Moved first: the frost follows its surface (the move is told before it happens).
            pump_for(Duration::from_millis(300));
            panel.move_to((PANEL.0 + 20, PANEL.1 + 20))?;
            flyout.move_to((FLYOUT.0 + 20, FLYOUT.1 + 20))?;
            pump_for(Duration::from_millis(800));
            capture(&path, (PANEL.0 + 20, PANEL.1 + 20, (FLYOUT.0 + FLYOUT.2 as i32 - PANEL.0) as u32, PANEL.3))?;
            let drivers = modules::gpu_drivers();
            println!("wrote {path}; GPU drivers loaded: {drivers:?}");
            println!("modules: {}", modules::loaded().join(" "));
            drop(stripes);
            return Ok(());
        }
        let end = Instant::now() + Duration::from_secs(60);
        while Instant::now() < end {
            pump_for(Duration::from_millis(250));
            while let Some((_, event)) = next_event() {
                if matches!(event, Event::Key(0x1b) | Event::Close) {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// One surface's frame: its body (glass when the system backdrop can show it) and content.
    fn draw(
        ui: &mut Ui,
        surface: &Surface,
        theme: &Theme,
        size: (u32, u32),
        view: fn(&mut Frame, &Theme, Rect),
        fill: bool,
    ) -> (DisplayList, Option<stillkit::Glass>) {
        let input = Interaction::default();
        let mut f = Frame::new(&mut ui.text, theme, 1.0, &input);
        let body = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);
        let glass = f.surface(body, surface.glass_available() && !fill);
        view(&mut f, theme, body);
        (f.list, glass)
    }

    fn control_panel(f: &mut Frame, t: &Theme, body: Rect) {
        let content = body.inset(Insets::all(f.px(t.metrics.pad)));
        let (head, rest) = content.take_top(f.px(32.0), f.px(10.0));
        f.text_in(head, "Control Center", TextRole::Title2, t.color.label, Align::Start);
        let mut col = Stack::column(rest, f.px(t.metrics.gap));
        let tiles = col.next(f.px(56.0) * 2.0 + f.px(8.0));
        let items = [
            (Icon::Wifi(3), "Wi-Fi", "Home 5G", true),
            (Icon::Bluetooth, "Bluetooth", "AirPods", true),
            (Icon::Moon, "Focus", "Off", false),
            (Icon::NightLight, "Night light", "Until 7:00", false),
        ];
        for (i, (icon, title, subtitle, on)) in items.into_iter().enumerate() {
            let r = grid(tiles, 2, f.px(8.0), f.px(56.0), i);
            f.tile(&Tile { id: 20 + i as u32, icon, title, subtitle, on, enabled: true, more: None }, r);
        }
        let sliders = col.next(f.px(108.0));
        let inner = f.card(sliders);
        let (label, rest) = inner.take_top(f.line(TextRole::Subheadline), f.px(4.0));
        f.text_in(label, "Display", TextRole::Subheadline, t.color.secondary, Align::Start);
        let (bar, rest) = rest.take_top(f.px(28.0), f.px(6.0));
        f.slider(30, bar, 0.7, "Brightness", Some(Icon::Brightness));
        f.slider(31, rest.take_top(f.px(28.0), 0.0).0, 0.4, "Volume", Some(Icon::Volume(2)));
        let toggles = col.next(f.px(t.metrics.row_single + 12.0));
        f.card(toggles);
        let row = toggles.inset(Insets::all(f.px(6.0)));
        f.row(
            &Row {
                id: 40,
                lead: Lead::Badge(Icon::Airplane, false),
                title: "Airplane mode",
                subtitle: None,
                accessory: Accessory::Switch(false),
                selected: false,
                enabled: true,
            },
            row,
        );
    }

    fn menu(f: &mut Frame, t: &Theme, body: Rect) {
        let inner = body.inset(Insets::all(f.px(6.0)));
        let mut rows = Stack::column(inner, 0.0);
        for (i, (icon, title)) in [
            (Icon::Settings, "Settings"),
            (Icon::Wifi(2), "Network"),
            (Icon::Bluetooth, "Devices"),
            (Icon::Moon, "Focus assist"),
            (Icon::Power, "Power"),
        ]
        .into_iter()
        .enumerate()
        {
            let r = rows.next(f.px(t.metrics.row_single));
            f.row(
                &Row {
                    id: 60 + i as u32,
                    lead: Lead::Icon(icon),
                    title,
                    subtitle: None,
                    accessory: Accessory::Chevron,
                    selected: i == 0,
                    enabled: true,
                },
                r,
            );
        }
    }

    /// Opaque stripes and a word, so the frost over them is plain to see.
    fn stripes(w: u32, h: u32) -> DisplayList {
        let colours = [
            Rgba::new(0.95, 0.30, 0.25, 1.0),
            Rgba::new(0.98, 0.85, 0.20, 1.0),
            Rgba::new(0.20, 0.70, 0.35, 1.0),
            Rgba::new(0.15, 0.40, 0.95, 1.0),
            Rgba::new(0.05, 0.05, 0.08, 1.0),
        ];
        let ops = (0..w.div_ceil(24))
            .map(|i| Op::Fill {
                rect: Rect::new(i as f32 * 24.0, 0.0, 24.0, h as f32),
                radius: 0.0,
                color: colours[i as usize % colours.len()],
            })
            .collect();
        DisplayList { ops, ..Default::default() }
    }

    fn pump_for(time: Duration) {
        let end = Instant::now() + time;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            // SAFETY: a standard message pump on this thread.
            unsafe {
                MsgWaitForMultipleObjectsEx(None, left.as_millis() as u32, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    /// The composed screen inside `rect` (this process's windows only, see `--shot`), as a PNG.
    fn capture(path: &str, (x, y, w, h): (i32, i32, u32, u32)) -> Result<()> {
        use windows::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
            CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits,
            ReleaseDC, SRCCOPY, SelectObject,
        };
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        // SAFETY: GDI objects made and released here; the buffer matches the bitmap header.
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, w as i32, h as i32);
            let old = SelectObject(dc, bitmap.into());
            BitBlt(dc, 0, 0, w as i32, h as i32, Some(screen), x, y, SRCCOPY | CAPTUREBLT).context("BitBlt")?;
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            GetDIBits(dc, bitmap, 0, h, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
        }
        for p in pixels.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
            p[3] = 255;
        }
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        Ok(())
    }
}
