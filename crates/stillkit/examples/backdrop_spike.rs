//! The system-blur mechanisms the system backdrop was chosen from (docs/ui.md, "System
//! backdrop"), each on a never-activated popup over an opaque striped window of this process,
//! captured from the screen inside that popup's rectangle only:
//!
//! - `dwm`: `DWMWA_SYSTEMBACKDROP_TYPE` = transient (acrylic), frame extended over the client;
//! - `accent`: the undocumented `SetWindowCompositionAttribute` acrylic accent;
//! - `layered`: a Windows.UI.Composition host backdrop in a *layered* window's own target,
//!   below its pixels (`isTopmost = false`), with a half-alpha white layered bitmap over it.
//!
//! `movebench` times one window move each way: `SetWindowPos` on a popup (with and without a
//! composition target), a layered surface's `move_to`, and a frosted surface's (the system
//! backdrop's `place` alone, and with the surface's own move).
//!
//! ```text
//! cargo run --release -p stillkit --example backdrop_spike -- dwm <png>
//! cargo run --release -p stillkit --example backdrop_spike -- movebench
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only.");
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    native::main()
}

#[cfg(windows)]
mod native {
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, bail};
    use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, Ui, modules};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Dwm::{
        DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_HOSTBACKDROPBRUSH,
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
    };
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::{BOOL, Interface, w};

    const AT: (i32, i32, i32, i32) = (360, 260, 360, 240);

    pub fn main() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.first().map(String::as_str) == Some("movebench") {
            return movebench();
        }
        let (Some(mode), Some(path)) = (args.first(), args.get(1)) else { bail!("usage: <dwm|accent|layered> <png> | movebench") };
        let mut ui = Ui::new()?;
        let mut stripes = Surface::new(Kind::Popup { activate: false }, "stripes", Role::Pane)?;
        let (w, h) = (AT.2 as u32 + 120, AT.3 as u32 + 120);
        stripes.present(&mut ui, (AT.0 - 60, AT.1 - 60), (w, h), striped(w, h))?;
        stripes.show();
        let before = modules::loaded();
        let started = Instant::now();
        let _keep: Box<dyn std::any::Any> = match mode.as_str() {
            "dwm" | "accent" => Box::new(plain(mode == "dwm")?),
            "layered" => Box::new(layered(&mut ui)?),
            _ => bail!("unknown mode {mode}"),
        };
        let made = started.elapsed();
        pump_for(Duration::from_millis(800));
        capture(path, AT)?;
        let new: Vec<String> = modules::loaded().into_iter().filter(|m| !before.contains(m)).collect();
        println!("{mode}: made in {:.1} ms; new modules: {new:?}; drivers: {:?}", made.as_secs_f64() * 1e3, modules::gpu_drivers());
        Ok(())
    }

    unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY: default handling.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// A plain (redirected, black) popup, never activated, with the DWM backdrop or the accent.
    fn plain(dwm: bool) -> Result<HWND> {
        #[repr(C)]
        struct Accent { state: u32, flags: u32, gradient: u32, animation: u32 }
        #[repr(C)]
        struct Data { attribute: u32, data: *mut std::ffi::c_void, size: usize }
        // SAFETY: plain Win32 calls on a window of our own.
        unsafe {
            RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(proc_), lpszClassName: w!("spike.plain"), ..Default::default() });
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("spike.plain"),
                w!("spike"),
                WS_POPUP,
                AT.0, AT.1, AT.2, AT.3,
                None, None, None, None,
            )?;
            DwmExtendFrameIntoClientArea(hwnd, &MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 })?;
            if dwm {
                let kind = DWMSBT_TRANSIENTWINDOW;
                DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, (&raw const kind).cast(), 4)?;
            } else {
                let user32 = windows::Win32::System::LibraryLoader::GetModuleHandleW(w!("user32.dll"))?;
                let f = windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("SetWindowCompositionAttribute")).context("no SetWindowCompositionAttribute")?;
                let f: unsafe extern "system" fn(HWND, *mut Data) -> BOOL = std::mem::transmute(f);
                // ACCENT_ENABLE_ACRYLICBLURBEHIND (4), a dark tint (AABBGGRR).
                let mut accent = Accent { state: 4, flags: 0, gradient: 0x5a1a1a1a, animation: 0 };
                let mut data = Data { attribute: 19, data: (&raw mut accent).cast(), size: size_of::<Accent>() };
                f(hwnd, &mut data).ok()?;
            }
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(hwnd)
        }
    }

    /// A layered surface with a host backdrop in its own target, below its (half-white) pixels.
    fn layered(ui: &mut Ui) -> Result<(Surface, windows::System::DispatcherQueueController, windows::UI::Composition::Compositor, windows::UI::Composition::Desktop::DesktopWindowTarget)> {
        use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
        use windows::Win32::System::WinRT::{CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions};
        let mut s = Surface::new(Kind::Popup { activate: false }, "layered", Role::Pane)?;
        let list = DisplayList {
            ops: vec![Op::Fill { rect: Rect::new(0.0, 0.0, AT.2 as f32, AT.3 as f32 / 2.0), radius: 0.0, color: Rgba::new(1.0, 1.0, 1.0, 0.5) }],
            ..Default::default()
        };
        s.present(ui, (AT.0, AT.1), (AT.2 as u32, AT.3 as u32), list)?;
        s.show();
        // SAFETY: WinRT and window calls on this thread's own objects.
        unsafe {
            let queue = CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            })?;
            let compositor = windows::UI::Composition::Compositor::new()?;
            let enable = BOOL::from(true);
            DwmSetWindowAttribute(s.hwnd(), DWMWA_USE_HOSTBACKDROPBRUSH, (&raw const enable).cast(), 4)?;
            let target = compositor.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(s.hwnd(), false)?;
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetBrush(&compositor.CreateHostBackdropBrush()?)?;
            sprite.SetRelativeSizeAdjustment(windows_numerics::Vector2 { X: 1.0, Y: 1.0 })?;
            target.SetRoot(&sprite)?;
            Ok((s, queue, compositor, target))
        }
    }

    /// What one window move costs this thread: a plain popup, the same popup with a
    /// composition target, a layered surface (`move_to`), and a frosted CPU surface.
    fn movebench() -> Result<()> {
        use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
        let time = |label: &str, mut step: Box<dyn FnMut(i32) -> Result<()>>| -> Result<()> {
            let mut times = Vec::new();
            for i in 0..600 {
                let t = Instant::now();
                step(i)?;
                times.push(t.elapsed().as_secs_f64() * 1e3);
                pump_for(Duration::from_millis(if i % 10 == 0 { 2 } else { 0 }));
            }
            times.sort_by(f64::total_cmp);
            println!("{label}: median {:.3} ms, p95 {:.3} ms", times[300], times[570]);
            Ok(())
        };
        let mv = |hwnd: HWND| move |i: i32| -> Result<()> {
            // SAFETY: our own window.
            unsafe { SetWindowPos(hwnd, None, 200 + i % 100, 200, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)? };
            Ok(())
        };
        let plain = plain_window()?;
        time("plain popup SetWindowPos", Box::new(mv(plain)))?;
        time(
            "plain popup SetWindowPos, no redraw/copy/changing",
            Box::new(move |i| {
                // SAFETY: our own window.
                unsafe {
                    SetWindowPos(plain, None, 200 + i % 100, 200, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW | SWP_NOCOPYBITS | SWP_NOSENDCHANGING | SWP_DEFERERASE)?
                };
                Ok(())
            }),
        )?;
        time(
            "plain popup DeferWindowPos (one window)",
            Box::new(move |i| {
                // SAFETY: our own window.
                unsafe {
                    let batch = BeginDeferWindowPos(1)?;
                    let batch = DeferWindowPos(batch, plain, None, 200 + i % 100, 200, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)?;
                    EndDeferWindowPos(batch)?;
                };
                Ok(())
            }),
        )?;
        // SAFETY: WinRT calls on this thread's own objects.
        let (_queue, _compositor, _target, targeted) = unsafe {
            use windows::Win32::System::WinRT::{CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions};
            let queue = CreateDispatcherQueueController(DispatcherQueueOptions { dwSize: size_of::<DispatcherQueueOptions>() as u32, threadType: DQTYPE_THREAD_CURRENT, apartmentType: DQTAT_COM_NONE })?;
            let compositor = windows::UI::Composition::Compositor::new()?;
            let hwnd = plain_window()?;
            let target = compositor.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(hwnd, true)?;
            let sprite = compositor.CreateSpriteVisual()?;
            sprite.SetBrush(&compositor.CreateColorBrushWithColor(windows::UI::Color { A: 255, R: 40, G: 40, B: 40 })?)?;
            sprite.SetRelativeSizeAdjustment(windows_numerics::Vector2 { X: 1.0, Y: 1.0 })?;
            target.SetRoot(&sprite)?;
            (queue, compositor, target, hwnd)
        };
        time("popup with a composition target SetWindowPos", Box::new(mv(targeted)))?;
        let mut ui = Ui::new()?;
        let mut layered = Surface::new(Kind::Popup { activate: false }, "layered", Role::Pane)?;
        layered.present(&mut ui, (200, 500), (360, 240), striped(360, 240))?;
        layered.show();
        time("layered surface move_to", Box::new(move |i| layered.move_to((200 + i % 100, 500))))?;
        let mut frosted = Surface::new(Kind::Popup { activate: false }, "frosted", Role::Pane)?;
        frosted.present(&mut ui, (700, 500), (360, 240), striped(360, 240))?;
        frosted.show();
        let body = Rect::new(0.0, 0.0, 360.0, 240.0);
        let material = stillkit::glass::GlassMaterial { clarity: 0.35, blur: 24.0, refraction: 0.0, highlight: 0.0, tint: Rgba::new(0.1, 0.1, 0.1, 0.4) };
        frosted.set_glass(Some(stillkit::Glass { body, radius: 16.0, material }));
        anyhow::ensure!(frosted.has_glass(), "no system frost");
        let hwnd = frosted.hwnd();
        let system = stillkit::platform::windows::glass::system();
        time("system frost place alone", Box::new({
            let system = system.clone();
            move |i| {
                use stillkit::glass::Backdrop;
                system.place(hwnd, stillkit::glass::Shape { x: 700 + i % 100, y: 500, width: 360, height: 240 }, 16.0);
                Ok(())
            }
        }))?;
        time("frosted surface move_to (place + ULW)", Box::new(move |i| frosted.move_to((700 + i % 100, 500))))?;
        Ok(())
    }

    fn plain_window() -> Result<HWND> {
        // SAFETY: plain Win32 calls on a window of our own.
        unsafe {
            RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(proc_), lpszClassName: w!("spike.bench"), ..Default::default() });
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP,
                w!("spike.bench"),
                w!("bench"),
                WS_POPUP,
                200, 200, 360, 240,
                None, None, None, None,
            )?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(hwnd)
        }
    }

    fn striped(w: u32, h: u32) -> DisplayList {
        let colours = [
            Rgba::new(0.95, 0.30, 0.25, 1.0),
            Rgba::new(0.98, 0.85, 0.20, 1.0),
            Rgba::new(0.20, 0.70, 0.35, 1.0),
            Rgba::new(0.15, 0.40, 0.95, 1.0),
            Rgba::new(0.05, 0.05, 0.08, 1.0),
        ];
        let ops = (0..w.div_ceil(24))
            .map(|i| Op::Fill { rect: Rect::new(i as f32 * 24.0, 0.0, 24.0, h as f32), radius: 0.0, color: colours[i as usize % colours.len()] })
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

    /// The composed screen inside `rect` (over this process's own stripes), as a PNG.
    fn capture(path: &str, (x, y, w, h): (i32, i32, i32, i32)) -> Result<()> {
        use windows::Win32::Graphics::Gdi::*;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        // SAFETY: GDI objects made and released here; the buffer matches the header.
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bitmap.into());
            BitBlt(dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT)?;
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
                ..Default::default()
            };
            GetDIBits(dc, bitmap, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
        }
        for p in pixels.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
            p[3] = 255;
        }
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.write_header()?.write_image_data(&pixels)?;
        Ok(())
    }
}
