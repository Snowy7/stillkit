//! The system backdrop measured (docs/ui.md, "System backdrop"): a 360×420 One UI panel, release
//! build, 10 s settle, 30 s idle, then 10 s of the app dragging its own window along a circle
//! every display frame (no input is sent), in each configuration:
//!
//! - `fill`: CPU mode, today's flat tinted fill;
//! - `system`: CPU mode, the system backdrop's frost (no backdrop installed);
//! - `gpu`: GPU mode with in-process glass, frost only (needs `--features gpu`).
//!
//! ```text
//! cargo run --release -p stillkit --features gpu --example backdrop_measure
//! cargo run --release -p stillkit --example backdrop_measure -- panel system   # one, by hand
//! ```
//!
//! The parent runs each as a child process and samples it from outside: private bytes, CPU time
//! and every thread's context switches. The child reports its notable graphics modules and its
//! own frame times (the present or move call).

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
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result};
    use stillkit::icons::Icon;
    use stillkit::interact::Interaction;
    use stillkit::kit::{Accessory, Frame, Lead, Row, Tile};
    use stillkit::layout::{Align, Insets, Stack, grid};
    use stillkit::platform::{Render, set_render};
    use stillkit::{DisplayList, Kind, Rect, Role, Scheme, Style, Surface, TextRole, Theme, Ticker, Ui, modules};
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, TranslateMessage,
    };

    const SETTLE: u64 = 10;
    const IDLE: u64 = 30;
    const DRAG: u64 = 10;
    const ORIGIN: (i32, i32) = (400, 260);
    const SIZE: (u32, u32) = (360, 420);
    /// Modules worth naming: the graphics stack, composition, and any vendor driver.
    const NOTABLE: &[&str] = &[
        "d2d1.dll", "d3d10warp.dll", "d3d11.dll", "d3d12.dll", "d3d12core.dll", "dxgi.dll",
        "dxcore.dll", "dcomp.dll", "coremessaging.dll", "windows.ui.dll",
    ];

    pub fn main() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
            (Some("panel"), Some(config)) => panel(config),
            _ => parent(&args),
        }
    }

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

    /// Context switches of each live thread of `pid` (by thread id): each one is a wake.
    fn switches(pid: u32) -> HashMap<u64, u64> {
        let mut buf = vec![0u8; 4 << 20];
        let mut ret = 0;
        loop {
            // SAFETY: the buffer and its length match; class 5 is SystemProcessInformation.
            let status = unsafe { NtQuerySystemInformation(5, buf.as_mut_ptr(), buf.len() as u32, &mut ret) };
            if status == 0 {
                break;
            }
            if buf.len() > 64 << 20 {
                return HashMap::new();
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
                return (0..threads).map(|i| at + 0x100 + i * 0x50).map(|t| (read(t + 0x30, 8), read(t + 0x40, 4))).collect();
            }
            if next == 0 {
                return HashMap::new();
            }
            at += next;
        }
    }

    fn wakes(a: &HashMap<u64, u64>, b: &HashMap<u64, u64>) -> u64 {
        b.iter().map(|(t, n)| n.saturating_sub(a.get(t).copied().unwrap_or(0))).sum()
    }

    /// Runs every configuration, or only those named (`fill`, `system`, `gpu`).
    fn parent(only: &[String]) -> Result<()> {
        let exe = std::env::current_exe()?;
        let mut configs = vec![("CPU, fill (today)", "fill"), ("CPU, system backdrop", "system")];
        if cfg!(feature = "gpu") {
            configs.push(("GPU, in-process glass (frost only)", "gpu"));
        }
        println!("| Configuration | Private MB | Idle wakes / 30 s | Idle CPU | Drag CPU | Drag frame median / p95 ms | Modules |");
        println!("| --- | --- | --- | --- | --- | --- | --- |");
        configs.retain(|(_, c)| only.is_empty() || only.iter().any(|o| o == c));
        for (label, config) in configs {
            let row = run(&exe, config).unwrap_or_else(|e| format!("failed: {e:#}"));
            println!("| {label} | {row} |");
        }
        Ok(())
    }

    fn run(exe: &std::path::Path, config: &str) -> Result<String> {
        let mut child = Command::new(exe).args(["panel", config]).stdout(Stdio::piped()).spawn()?;
        let result = (|| -> Result<String> {
            let mut lines = BufReader::new(child.stdout.take().context("stdout")?).lines();
            let mut next = || -> Result<String> { lines.next().context("the panel ended early")?.map_err(Into::into) };
            let ready = next()?;
            let words: Vec<&str> = ready.split('\t').collect();
            anyhow::ensure!(words.first() == Some(&"ready"), "panel said {ready:?}");
            let pid: u32 = words[1].parse()?;
            let process = open(pid)?;
            std::thread::sleep(Duration::from_secs(SETTLE));
            let (cpu_a, sw_a) = (cpu(process), switches(pid));
            let drag = next()?;
            anyhow::ensure!(drag.starts_with("drag"), "panel said {drag:?}");
            let (cpu_b, sw_b) = (cpu(process), switches(pid));
            let private = private_mb(process);
            let modules = drag.trim_start_matches("drag\t").to_string();
            let frames = next()?;
            let cpu_c = cpu(process);
            // SAFETY: opened above.
            let _ = unsafe { CloseHandle(process) };
            Ok(format!(
                "**{private:.1}** | **{}** | {:.3} % | {:.1} % | {} | {modules}",
                wakes(&sw_a, &sw_b),
                (cpu_b - cpu_a).as_secs_f64() / IDLE as f64 * 100.0,
                (cpu_c - cpu_b).as_secs_f64() / DRAG as f64 * 100.0,
                frames.trim_start_matches("frames\t"),
            ))
        })();
        let _ = child.kill();
        let _ = child.wait();
        result
    }

    fn pump(timeout: Duration) {
        // SAFETY: a standard message pump on this thread.
        unsafe {
            MsgWaitForMultipleObjectsEx(None, timeout.as_millis() as u32, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn panel(config: &str) -> Result<()> {
        set_render(if config == "gpu" { Render::Gpu } else { Render::Cpu });
        let mut ui = Ui::new()?;
        let theme = Theme::new(Style::OneUi, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
        let mut surface = Surface::new(Kind::Popup { activate: false }, "One UI panel", Role::Dialog)?;
        anyhow::ensure!((surface.render() == Render::Gpu) == (config == "gpu"), "GPU mode unavailable");
        let glass = config != "fill" && surface.glass_available();
        anyhow::ensure!(glass || config == "fill", "no glass for {config}");
        let (list, body) = view(&mut ui, &theme, glass);
        surface.present(&mut ui, ORIGIN, SIZE, list)?;
        surface.show();
        surface.set_glass(body);
        let loaded = modules::loaded();
        let mut notable: Vec<&str> = NOTABLE.iter().copied().filter(|m| loaded.iter().any(|l| l == m)).collect();
        let drivers = modules::gpu_drivers();
        notable.extend(drivers.iter().map(String::as_str));
        let notable = notable.join(", ");
        let mut out = std::io::stdout();
        writeln!(out, "ready\t{}", std::process::id())?;
        out.flush()?;
        let start = Instant::now();
        while let Some(left) = Duration::from_secs(SETTLE + IDLE).checked_sub(start.elapsed()) {
            pump(left);
        }
        // Modules once more after the idle (anything loaded late counts).
        let late: Vec<String> = modules::gpu_drivers().into_iter().filter(|d| !drivers.contains(d)).collect();
        writeln!(out, "drag\t{notable}{}", if late.is_empty() { String::new() } else { format!(" (late: {})", late.join(", ")) })?;
        out.flush()?;
        // The drag: the window moves along a circle every display frame.
        let mut ticker = Ticker::new()?;
        let mut times = Vec::new();
        let begun = Instant::now();
        while begun.elapsed() < Duration::from_secs(DRAG) {
            ticker.schedule(true);
            // SAFETY: our own timer handle.
            unsafe { WaitForSingleObject(ticker.handle(), 100) };
            pump(Duration::ZERO);
            let a = begun.elapsed().as_secs_f32() * std::f32::consts::TAU / 2.0;
            let at = (ORIGIN.0 + (a.cos() * 120.0) as i32, ORIGIN.1 + (a.sin() * 80.0) as i32);
            let t = Instant::now();
            surface.move_to(at)?;
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        let q = |p: f64| times[((times.len() - 1) as f64 * p) as usize];
        writeln!(out, "frames\t{:.3} / {:.3} ({} frames)", q(0.5), q(0.95), times.len())?;
        out.flush()?;
        Ok(())
    }

    /// The panel: Control Center tiles, two sliders and a switch row over its body.
    fn view(ui: &mut Ui, t: &Theme, glass: bool) -> (DisplayList, Option<stillkit::Glass>) {
        let input = Interaction::default();
        let mut f = Frame::new(&mut ui.text, t, 1.0, &input);
        let body = Rect::new(0.0, 0.0, SIZE.0 as f32, SIZE.1 as f32);
        let glass = f.surface(body, glass);
        let content = body.inset(Insets::all(f.px(t.metrics.pad)));
        let (head, rest) = content.take_top(f.px(32.0), f.px(10.0));
        f.text_in(head, "Control Center", TextRole::Title2, t.color.label, Align::Start);
        let mut col = Stack::column(rest, f.px(t.metrics.gap));
        let tiles = col.next(f.px(56.0) * 2.0 + f.px(8.0));
        for (i, (icon, title, subtitle, on)) in [
            (Icon::Wifi(3), "Wi-Fi", "Home 5G", true),
            (Icon::Bluetooth, "Bluetooth", "AirPods", true),
            (Icon::Moon, "Focus", "Off", false),
            (Icon::NightLight, "Night light", "Until 7:00", false),
        ]
        .into_iter()
        .enumerate()
        {
            let r = grid(tiles, 2, f.px(8.0), f.px(56.0), i);
            f.tile(&Tile { id: 20 + i as u32, icon, title, subtitle, on, enabled: true, more: None }, r);
        }
        let inner = f.card(col.next(f.px(108.0)));
        let (label, rest) = inner.take_top(f.line(TextRole::Subheadline), f.px(4.0));
        f.text_in(label, "Display", TextRole::Subheadline, t.color.secondary, Align::Start);
        let (bar, rest) = rest.take_top(f.px(28.0), f.px(6.0));
        f.slider(30, bar, 0.7, "Brightness", Some(Icon::Brightness));
        f.slider(31, rest.take_top(f.px(28.0), 0.0).0, 0.4, "Volume", Some(Icon::Volume(2)));
        let toggles = col.next(f.px(t.metrics.row_single + 12.0));
        f.card(toggles);
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
            toggles.inset(Insets::all(f.px(6.0))),
        );
        (f.list, glass)
    }
}
