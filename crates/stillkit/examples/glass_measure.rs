//! Render modes measured (docs/ui.md, "Render modes"): one small panel with glass, in each
//! configuration, release build, 30 s idle after a 10 s settle, then 10 s of a slider dragged by
//! the app itself (no input is sent).
//!
//! ```text
//! cargo run --release -p stillkit --features gpu --example glass_measure            # every config
//! cargo run --release -p stillkit --features gpu --example glass_measure -- panel --render gpu --glass inproc --power low
//! cargo run --release -p stillkit --features gpu --example glass_measure -- mixed --power low
//! ```
//!
//! One binary, three roles. The parent runs each configuration as a child `panel` process and
//! samples it (and its `host`, when glass is drawn by a separate process) from outside: private
//! bytes, CPU time, context switches (every thread's, drivers' included), and per-process GPU
//! memory from the `GPU Process Memory` counters. `host` is the separate glass process: a window of
//! its own under the panel, a wgpu device, the same glass code (`gpu_glass`), and no IPC beyond
//! its command line. `mixed` times small CPU and GPU surfaces in a process that already holds a
//! GPU device.

#[cfg(not(windows))]
fn main() {
    eprintln!("This measurement needs Windows.");
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    native::main()
}

#[cfg(windows)]
mod native {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result};
    use stillkit::glass::{Glass, GlassMaterial, GlassStyle};
    use stillkit::platform::windows::{gpu, gpu_glass};
    use stillkit::platform::{Render, set_render};
    use stillkit::{DisplayList, Kind, Op, Rect, Rgba, Role, Surface, TextStyle, Ticker, Ui};
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        WaitForSingleObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, TranslateMessage,
    };

    const SETTLE: u64 = 10;
    const IDLE: u64 = 30;
    const DRAG: u64 = 10;
    /// The panel: its window, and the glass body inside it (window px).
    const ORIGIN: (i32, i32) = (240, 240);
    const SIZE: (u32, u32) = (360, 220);
    const BODY: Rect = Rect::new(16.0, 16.0, 328.0, 188.0);
    const RADIUS: f32 = 22.0;
    const MATERIAL: GlassMaterial = GlassMaterial {
        clarity: 0.8,
        blur: 8.0,
        refraction: 0.6,
        highlight: 0.5,
        tint: Rgba::TRANSPARENT,
    };

    pub fn main() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
        let power = match flag("--power") {
            Some("high") => gpu::Power::High,
            _ => gpu::Power::Low,
        };
        gpu::set_power(power);
        match args.first().map(String::as_str) {
            Some("panel") => panel(flag("--render") == Some("gpu"), flag("--glass").unwrap_or("none")),
            Some("host") => host(flag("--below").context("--below")?.parse()?),
            Some("mixed") => mixed(),
            _ => parent(),
        }
    }

    // ---- what the parent measures, from outside the process -------------------------------

    fn open(pid: u32) -> Result<HANDLE> {
        // SAFETY: a query handle, closed by the caller.
        Ok(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)? })
    }

    fn private_mb(process: HANDLE) -> f64 {
        let mut c = PROCESS_MEMORY_COUNTERS_EX::default();
        // SAFETY: the struct and its size match.
        let _ = unsafe { GetProcessMemoryInfo(process, (&raw mut c).cast(), size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32) };
        c.PrivateUsage as f64 / (1024.0 * 1024.0)
    }

    fn cpu(process: HANDLE) -> Duration {
        let [mut a, mut b, mut kernel, mut user] = [FILETIME::default(); 4];
        // SAFETY: four local out-structs.
        let _ = unsafe { GetProcessTimes(process, &mut a, &mut b, &mut kernel, &mut user) };
        let t = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
        Duration::from_nanos((t(kernel) + t(user)) * 100)
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(class: u32, info: *mut u8, len: u32, ret: *mut u32) -> i32;
    }

    /// Context switches of each live thread of `pid` so far (by thread id): each one is a wake.
    /// Per thread, because a thread that exits takes its count with it.
    fn switches(pid: u32) -> std::collections::HashMap<u64, u64> {
        let mut buf = vec![0u8; 4 << 20];
        let mut ret = 0;
        loop {
            // SAFETY: the buffer and its length match; class 5 is SystemProcessInformation.
            let status = unsafe { NtQuerySystemInformation(5, buf.as_mut_ptr(), buf.len() as u32, &mut ret) };
            if status == 0 {
                break;
            }
            if buf.len() > 64 << 20 {
                return Default::default();
            }
            buf.resize(buf.len() * 2, 0);
        }
        let read = |at: usize, n: usize| -> u64 {
            let mut v = [0u8; 8];
            v[..n].copy_from_slice(&buf[at..at + n]);
            u64::from_le_bytes(v)
        };
        let mut at = 0;
        loop {
            // SYSTEM_PROCESS_INFORMATION (x64): next 0, threads 4, pid 0x50; threads from 0x100,
            // 0x50 bytes each: thread id at 0x30, ContextSwitches at 0x40.
            let (next, threads, id) = (read(at, 4) as usize, read(at + 4, 4) as usize, read(at + 0x50, 8));
            if id == u64::from(pid) {
                return (0..threads)
                    .map(|i| at + 0x100 + i * 0x50)
                    .map(|t| (read(t + 0x30, 8), read(t + 0x40, 4)))
                    .collect();
            }
            if next == 0 {
                return Default::default();
            }
            at += next;
        }
    }

    /// Per-process GPU memory (dedicated, shared) in MB, summed over adapters.
    fn gpu_mb(pids: &[u32]) -> Vec<(f64, f64)> {
        use windows::Win32::System::Performance::{
            PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_LARGE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW,
            PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
        };
        let mut out = vec![(0.0, 0.0); pids.len()];
        // SAFETY: a PDH query of our own, closed at the end; item buffers sized as PDH asks.
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != 0 {
                return out;
            }
            let mut counters = [PDH_HCOUNTER::default(); 2];
            for (c, path) in counters.iter_mut().zip([
                windows::core::w!("\\GPU Process Memory(*)\\Dedicated Usage"),
                windows::core::w!("\\GPU Process Memory(*)\\Shared Usage"),
            ]) {
                PdhAddEnglishCounterW(query, path, 0, c);
            }
            PdhCollectQueryData(query);
            for (k, counter) in counters.iter().enumerate() {
                let (mut size, mut count) = (0u32, 0u32);
                PdhGetFormattedCounterArrayW(*counter, PDH_FMT_LARGE, &mut size, &mut count, None);
                let mut items = vec![0u8; size as usize];
                if PdhGetFormattedCounterArrayW(*counter, PDH_FMT_LARGE, &mut size, &mut count, Some(items.as_mut_ptr().cast())) != 0 {
                    continue;
                }
                let items = std::slice::from_raw_parts(items.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(), count as usize);
                for item in items {
                    let name = item.szName.to_string().unwrap_or_default();
                    for (i, pid) in pids.iter().enumerate() {
                        if name.starts_with(&format!("pid_{pid}_")) {
                            let mb = item.FmtValue.Anonymous.largeValue as f64 / (1024.0 * 1024.0);
                            if k == 0 { out[i].0 += mb } else { out[i].1 += mb }
                        }
                    }
                }
            }
            PdhCloseQuery(query);
        }
        out
    }

    struct Sample {
        cpu: Duration,
        switches: std::collections::HashMap<u64, u64>,
    }

    fn sample(handles: &[(u32, HANDLE)]) -> Vec<Sample> {
        handles.iter().map(|&(pid, h)| Sample { cpu: cpu(h), switches: switches(pid) }).collect()
    }

    fn parent() -> Result<()> {
        let exe = std::env::current_exe()?;
        let configs: &[(&str, &str, &str, &str)] = &[
            ("CPU only (fill)", "cpu", "none", "low"),
            ("CPU + glass process", "cpu", "host", "low"),
            ("CPU + glass process", "cpu", "host", "high"),
            ("GPU, no glass", "gpu", "none", "low"),
            ("GPU, no glass", "gpu", "none", "high"),
            ("GPU + in-process glass", "gpu", "inproc", "low"),
            ("GPU + in-process glass", "gpu", "inproc", "high"),
        ];
        println!("| Configuration | Adapter | Private MB (app + host) | GPU dedicated / shared MB | Idle wakes / 30 s | Idle CPU | Drag CPU | Drag frame median / p95 ms |");
        println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
        for &(label, render, glass, power) in configs {
            let row = run(&exe, label, render, glass, power).unwrap_or_else(|e| format!("| {label} | {power} | failed: {e:#} |"));
            println!("{row}");
        }
        Ok(())
    }

    fn run(exe: &std::path::Path, label: &str, render: &str, glass: &str, power: &str) -> Result<String> {
        let mut child = Command::new(exe)
            .args(["panel", "--render", render, "--glass", glass, "--power", power])
            .stdout(Stdio::piped())
            .spawn()?;
        let result = (|| -> Result<String> {
            let mut lines = BufReader::new(child.stdout.take().context("stdout")?).lines();
            let mut next = || -> Result<String> { lines.next().context("the panel ended early")?.map_err(Into::into) };
            let ready = next()?;
            let words: Vec<&str> = ready.split('\t').collect();
            anyhow::ensure!(words.first() == Some(&"ready"), "panel said {ready:?}");
            let pids: Vec<u32> = [words[1], words[2]].iter().filter_map(|w| w.parse().ok()).filter(|&p| p != 0).collect();
            let adapter = words.get(3).copied().unwrap_or("").to_string();
            let handles: Vec<(u32, HANDLE)> = pids.iter().map(|&p| Ok((p, open(p)?))).collect::<Result<_>>()?;
            std::thread::sleep(Duration::from_secs(SETTLE));
            let a = sample(&handles);
            let drag = next()?;
            anyhow::ensure!(drag == "drag", "panel said {drag:?}");
            let b = sample(&handles);
            let private: Vec<f64> = handles.iter().map(|&(_, h)| private_mb(h)).collect();
            let gpu = gpu_mb(&pids);
            let frames = next()?;
            let c = sample(&handles);
            let wakes: Vec<u64> = a.iter().zip(&b).map(|(a, b)| b.switches.iter().map(|(t, n)| n.saturating_sub(a.switches.get(t).copied().unwrap_or(0))).sum()).collect();
            let idle: Vec<f64> = a.iter().zip(&b).map(|(a, b)| (b.cpu - a.cpu).as_secs_f64() / IDLE as f64 * 100.0).collect();
            let dragged: Vec<f64> = b.iter().zip(&c).map(|(b, c)| (c.cpu - b.cpu).as_secs_f64() / DRAG as f64 * 100.0).collect();
            for (_, h) in handles {
                // SAFETY: handles opened above.
                let _ = unsafe { CloseHandle(h) };
            }
            let both = |v: &[f64], unit: &str, digits: usize| {
                let sum: f64 = v.iter().sum();
                if v.len() > 1 {
                    format!("**{sum:.digits$}{unit}** ({:.digits$} + {:.digits$})", v[0], v[1])
                } else {
                    format!("**{sum:.digits$}{unit}**")
                }
            };
            let ded: Vec<f64> = gpu.iter().map(|g| g.0).collect();
            let shared: Vec<f64> = gpu.iter().map(|g| g.1).collect();
            let wakes_f: Vec<f64> = wakes.iter().map(|&w| w as f64).collect();
            Ok(format!(
                "| {label} | {adapter} | {} | {} / {} | {} | {} | {} | {} |",
                both(&private, "", 1),
                both(&ded, "", 1),
                both(&shared, "", 1),
                both(&wakes_f, "", 0),
                both(&idle, " %", 3),
                both(&dragged, " %", 1),
                frames.trim_start_matches("frames\t"),
            ))
        })();
        let _ = child.kill();
        let _ = child.wait();
        result
    }

    // ---- the app: one panel -----------------------------------------------------------------

    /// The panel's frame: its body (glass, or a drawn fill), a title, a slider at `value`.
    fn view(ui: &mut Ui, value: f32, glass: bool) -> DisplayList {
        let title = TextStyle::new("Segoe UI Variable Text", 15.0);
        let label = TextStyle::new("Segoe UI Variable Text", 13.0);
        let (tw, th) = ui.text.measure("Brightness", &title);
        let percent = format!("{:.0}%", value * 100.0);
        let (pw, ph) = ui.text.measure(&percent, &label);
        let track = Rect::new(40.0, 120.0, 280.0, 6.0);
        let knob = track.left + track.width() * value;
        let ink = Rgba::new(1.0, 1.0, 1.0, 0.95);
        DisplayList {
            ops: vec![
                Op::Fill {
                    rect: BODY,
                    radius: RADIUS,
                    color: if glass { Rgba::HIT } else { Rgba::new(0.12, 0.12, 0.13, 0.92) },
                },
                Op::Text { rect: Rect::new(40.0, 48.0, tw, th), text: "Brightness".into(), style: title, color: ink },
                Op::Fill { rect: track, radius: 3.0, color: Rgba::new(1.0, 1.0, 1.0, 0.25) },
                Op::Fill { rect: Rect::new(track.left, track.top, knob - track.left, 6.0), radius: 3.0, color: Rgba::new(0.2, 0.55, 1.0, 1.0) },
                Op::Fill { rect: Rect::new(knob - 11.0, track.top - 8.0, 22.0, 22.0), radius: 11.0, color: ink },
                Op::Text { rect: Rect::new(320.0 - pw, 148.0, pw, ph), text: percent.into(), style: label, color: ink },
            ],
            ..Default::default()
        }
    }

    /// Dispatches waiting messages after sleeping up to `timeout`; returns whether it woke early.
    fn pump(timeout: Duration) -> bool {
        // SAFETY: a standard message pump on this thread.
        unsafe {
            let woke = MsgWaitForMultipleObjectsEx(None, timeout.as_millis() as u32, QS_ALLINPUT, MWMO_INPUTAVAILABLE).0 == 0;
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            woke
        }
    }

    fn panel(gpu_mode: bool, glass: &str) -> Result<()> {
        set_render(if gpu_mode { Render::Gpu } else { Render::Cpu });
        let mut ui = Ui::new()?;
        let mut surface = Surface::new(Kind::Popup { activate: false }, "glass panel", Role::Dialog)?;
        anyhow::ensure!((surface.render() == Render::Gpu) == gpu_mode, "GPU mode unavailable");
        let with_glass = glass != "none";
        let first = view(&mut ui, 0.5, with_glass);
        surface.present(&mut ui, ORIGIN, SIZE, first)?;
        surface.show();
        let mut host: Option<Child> = None;
        match glass {
            "inproc" => {
                surface.set_glass_style(GlassStyle::default());
                surface.set_glass(Some(Glass { body: BODY, radius: RADIUS, material: MATERIAL }));
            }
            "host" => {
                let mut child = Command::new(std::env::current_exe()?)
                    .args(["host", "--below", &(surface.hwnd().0 as isize).to_string(), "--power", if gpu::power() == gpu::Power::High { "high" } else { "low" }])
                    .stdout(Stdio::piped())
                    .spawn()?;
                let mut line = String::new();
                BufReader::new(child.stdout.take().context("host stdout")?).read_line(&mut line)?;
                anyhow::ensure!(line.starts_with("ready"), "host said {line:?}");
                host = Some(child);
            }
            _ => {}
        }
        let adapter = if gpu_mode || glass == "host" {
            gpu::adapter(gpu::power()).map_or_else(|| "none".into(), |a| a.name)
        } else {
            "none (WARP)".into()
        };
        let host_pid = host.as_ref().map_or(0, Child::id);
        let mut out = std::io::stdout();
        writeln!(out, "ready\t{}\t{host_pid}\t{adapter}", std::process::id())?;
        out.flush()?;
        // Idle: asleep in the message wait, waking only for what arrives.
        let start = Instant::now();
        let idle_end = Duration::from_secs(SETTLE + IDLE);
        while let Some(left) = idle_end.checked_sub(start.elapsed()) {
            pump(left);
        }
        writeln!(out, "drag")?;
        out.flush()?;
        // The drag: the app moves its own slider every display frame (no input is sent).
        let mut ticker = Ticker::new()?;
        let mut times = Vec::new();
        let begun = Instant::now();
        while begun.elapsed() < Duration::from_secs(DRAG) {
            ticker.schedule(true);
            // SAFETY: our own timer handle.
            unsafe { WaitForSingleObject(ticker.handle(), 100) };
            pump(Duration::ZERO);
            let t = begun.elapsed().as_secs_f32();
            let value = 0.5 + 0.45 * (t * std::f32::consts::TAU / 2.0).sin();
            let list = view(&mut ui, value, with_glass);
            let at = Instant::now();
            surface.present(&mut ui, ORIGIN, SIZE, list)?;
            times.push(at.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        let q = |p: f64| times[((times.len() - 1) as f64 * p) as usize];
        writeln!(out, "frames\t{:.3} / {:.3} ({} frames)", q(0.5), q(0.95), times.len())?;
        out.flush()?;
        if let Some(mut host) = host {
            let _ = host.kill();
            let _ = host.wait();
        }
        Ok(())
    }

    // ---- the separate glass process ---------------------------------------------------------

    unsafe extern "system" fn host_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY: default handling of the message we were called with.
        unsafe { windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// A window of its own just under `below` (the panel), with the panel's glass drawn by the
    /// same code as in-process glass, on its own wgpu device. Composition only: no D3D11.
    fn host(below: isize) -> Result<()> {
        use windows::Win32::Graphics::DirectComposition::{
            DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionVisual,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, RegisterClassW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetWindowPos, ShowWindow,
            WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_EX_TRANSPARENT, WS_POPUP,
        };
        use windows::core::{Interface, w};
        let below = HWND(below as _);
        // SAFETY: plain Win32 window and composition setup on this thread.
        let (hwnd, dcomp, _target, glass) = unsafe {
            let class = WNDCLASSW { lpfnWndProc: Some(host_proc), lpszClassName: w!("stillkit-glass-host"), ..Default::default() };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                w!("stillkit-glass-host"),
                w!("glass host"),
                WS_POPUP,
                ORIGIN.0,
                ORIGIN.1,
                SIZE.0 as i32,
                SIZE.1 as i32,
                None,
                None,
                None,
                None,
            )?;
            let dcomp: IDCompositionDesktopDevice = DCompositionCreateDevice2(None)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            target.SetRoot(&visual)?;
            let device = gpu_glass::device().context("glass device")?;
            let mut glass = gpu_glass::GlassLayer::new(device, visual.cast::<IDCompositionVisual>()?, SIZE, 1.0)?;
            glass.place(ORIGIN);
            glass.set(&[Glass { body: BODY, radius: RADIUS, material: MATERIAL }], &GlassStyle::default(), 0.0);
            dcomp.Commit()?;
            SetWindowPos(hwnd, Some(below), ORIGIN.0, ORIGIN.1, SIZE.0 as i32, SIZE.1 as i32, SWP_NOACTIVATE)?;
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            (hwnd, dcomp, target, glass)
        };
        let mut glass = glass;
        // Draw until the glass has faded in, then sleep until killed.
        let mut ticker = Ticker::new()?;
        while glass.render() {
            ticker.schedule(true);
            // SAFETY: our own timer handle.
            unsafe { WaitForSingleObject(ticker.handle(), 100) };
        }
        let _ = (&hwnd, &dcomp);
        println!("ready");
        std::io::stdout().flush()?;
        loop {
            pump(Duration::from_secs(3600));
        }
    }

    // ---- CPU surfaces beside a GPU device ---------------------------------------------------

    /// What a small CPU surface costs in a process that already holds a GPU device, against the
    /// same surface on the GPU: private memory each adds, and redraw time.
    fn mixed() -> Result<()> {
        let mut ui = Ui::new()?;
        // The GPU panel with glass: the device and glass are up.
        let mut panel = Surface::with_render(Kind::Popup { activate: false }, "panel", Role::Dialog, Render::Gpu)?;
        let first = view(&mut ui, 0.5, true);
        panel.present(&mut ui, ORIGIN, SIZE, first)?;
        panel.show();
        panel.set_glass(Some(Glass { body: BODY, radius: RADIUS, material: MATERIAL }));
        for _ in 0..30 {
            pump(Duration::from_millis(16));
        }
        // SAFETY: the pseudo handle of this process.
        let me = unsafe { GetCurrentProcess() };
        let widget = |ui: &mut Ui, n: u32| {
            let style = TextStyle::new("Segoe UI Variable Text", 13.0);
            let text = format!("12:{:02}", n % 60);
            let (w, h) = ui.text.measure(&text, &style);
            DisplayList {
                ops: vec![
                    Op::Fill { rect: Rect::new(0.0, 0.0, 200.0, 40.0), radius: 10.0, color: Rgba::new(0.1, 0.1, 0.1, 0.9) },
                    Op::Text { rect: Rect::new(12.0, 10.0, w, h), text: text.into(), style, color: Rgba::new(1.0, 1.0, 1.0, 1.0) },
                ],
                ..Default::default()
            }
        };
        for render in [Render::Cpu, Render::Gpu, Render::Cpu] {
            let before = private_mb(me);
            let mut s = Surface::with_render(Kind::Popup { activate: false }, "widget", Role::Toolbar, render)?;
            let first = widget(&mut ui, 0);
            s.present(&mut ui, (240, 480), (200, 40), first)?;
            s.show();
            let mut times = Vec::new();
            let cpu0 = cpu(me);
            for n in 1..=300 {
                let list = widget(&mut ui, n);
                let at = Instant::now();
                s.present(&mut ui, (240, 480), (200, 40), list)?;
                times.push(at.elapsed().as_secs_f64() * 1000.0);
                pump(Duration::ZERO);
            }
            let spent = cpu(me) - cpu0;
            times.sort_by(f64::total_cmp);
            println!(
                "{render:?} 200x40 surface beside a GPU device: +{:.2} MB private, redraw median {:.3} ms, p95 {:.3} ms, {:.1} ms CPU for 300 redraws",
                private_mb(me) - before,
                times[150],
                times[285],
                spent.as_secs_f64() * 1000.0
            );
        }
        println!("adapter: {}", gpu::adapter(gpu::power()).map_or_else(|| "none".into(), |a| a.name));
        Ok(())
    }
}
