//! Hosting a view: [`run`] opens a surface and drives a [`Runtime`] from its events, asleep
//! in the message wait whenever nothing happens (no timer, no ticker). The same binary is the
//! agent tooling, headless on a virtual clock:
//!
//! ```text
//! app --snap out.png [--scale 2] [--at 1.5]   render the frame at T seconds
//! app --tree out.json                          dump every element (key, path, role, rect...)
//! app --script s.json [--snap ..] [--tree ..]  replay timed input first
//! app --inactive                               a window that never takes the focus
//! ```
//!
//! Scripts and dumps use design pixels. A script is a JSON array of steps, each with an `at`
//! time in seconds and one input: `{"at": 0.2, "move": [x, y]}`, `"down"`, `"up"`,
//! `{"click": "key"}` (the centre of a keyed element), `{"key": "Tab"}` (also `Shift+Tab`,
//! `Enter`, `Space`, `Escape`, arrows `Left` `Right` `Up` `Down`, or a virtual-key number).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use accesskit::Role;
use anyhow::{Context, Result, bail};

use super::json::{Json, parse, quote};
use super::{El, Runtime};
use crate::{Chrome, Event, Kind, Rect, Rgba, Surface, Theme, next_event};

/// An app to [`run`]: its window and its view.
pub struct App<S, V> {
    pub title: &'static str,
    /// The client size in design pixels.
    pub size: (f32, f32),
    pub kind: Kind,
    pub theme: Theme,
    pub state: S,
    pub view: V,
}

/// One timed input of a [`Script`].
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Move(f32, f32),
    Down(f32, f32),
    Up(f32, f32),
    /// Down and up at the centre of the element with this key.
    Click(String),
    /// A virtual-key code; Tab steps the focus (backwards with `shift`).
    Key { vk: u16, shift: bool },
}

/// Timed input in design pixels, replayed on a runtime's virtual clock ([`Runtime::play`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Script(pub Vec<(Duration, Input)>);

impl Script {
    /// Reads the `--script` JSON format (see the module docs).
    pub fn parse(text: &str) -> Result<Self> {
        let Json::Arr(steps) = parse(text)? else {
            bail!("a script is an array of steps");
        };
        let mut out = Vec::with_capacity(steps.len());
        for (i, step) in steps.iter().enumerate() {
            let at = step.get("at").and_then(Json::num).unwrap_or(0.0);
            let point = |name| step.get(name).and_then(Json::point);
            let input = if let Some((x, y)) = point("move") {
                Input::Move(x, y)
            } else if let Some((x, y)) = point("down") {
                Input::Down(x, y)
            } else if let Some((x, y)) = point("up") {
                Input::Up(x, y)
            } else if let Some(key) = step.get("click").and_then(Json::str) {
                Input::Click(key.to_owned())
            } else if let Some(key) = step.get("key") {
                key_of(key).with_context(|| format!("step {i}: unknown key {key:?}"))?
            } else {
                bail!("step {i}: no input (move, down, up, click or key)");
            };
            out.push((Duration::from_secs_f64(at.max(0.0)), input));
        }
        out.sort_by_key(|(at, _)| *at);
        Ok(Self(out))
    }

    /// When the last step happens.
    pub fn end(&self) -> Duration {
        self.0.last().map_or(Duration::ZERO, |(at, _)| *at)
    }
}

fn key_of(key: &Json) -> Option<Input> {
    if let Some(vk) = key.num() {
        return Some(Input::Key { vk: vk as u16, shift: false });
    }
    let name = key.str()?;
    let (shift, name) = name.strip_prefix("Shift+").map_or((false, name), |rest| (true, rest));
    let vk = match name {
        "Tab" => 0x09,
        "Enter" => 0x0D,
        "Escape" => 0x1B,
        "Space" => 0x20,
        "Left" => 0x25,
        "Up" => 0x26,
        "Right" => 0x27,
        "Down" => 0x28,
        _ => return None,
    };
    Some(Input::Key { vk, shift })
}

impl<S> Runtime<S> {
    /// Replays `script` from the current virtual time: the clock advances to each step, the
    /// input goes in, and a frame runs, so handlers and views run as they would live. Stops
    /// after the last step at or before `until`, then advances the clock to `until`.
    pub fn play(&mut self, script: &Script, until: Option<Duration>) {
        let start = self.now();
        let s = self.scale();
        for (at, input) in &script.0 {
            if until.is_some_and(|until| *at > until) {
                break;
            }
            self.frame((start + *at).saturating_sub(self.now()));
            match input {
                Input::Move(x, y) => self.pointer_move(x * s, y * s),
                Input::Down(x, y) => self.pointer_down(x * s, y * s),
                Input::Up(x, y) => self.pointer_up(x * s, y * s),
                Input::Click(key) => {
                    if !self.click(key) {
                        log::warn!("script: no element keyed {key:?}");
                    }
                }
                Input::Key { vk: 0x09, shift } => self.tab(*shift),
                Input::Key { vk, .. } => self.key(*vk),
            }
            self.frame(Duration::ZERO);
        }
        if let Some(until) = until {
            self.frame((start + until).saturating_sub(self.now()));
        }
    }

    /// The `--tree` dump: every element as shown now, in design pixels.
    pub fn tree_json(&self) -> String {
        let s = self.scale();
        let (w, h) = self.size();
        let opt = |v: Option<String>| v.unwrap_or_else(|| "null".into());
        let nodes: Vec<String> = self
            .tree()
            .into_iter()
            .map(|n| {
                let r = n.rect;
                format!(
                    "{{\"key\":{},\"path\":{},\"role\":{},\"label\":{},\"rect\":[{},{},{},{}],\"fill\":{},\"text\":{},\"checked\":{},\"state\":\"{:?}\",\"focused\":{}}}",
                    opt(n.key.map(|k| quote(&k))),
                    quote(if n.path.is_empty() { "/" } else { &n.path }),
                    opt(n.role.map(|r| quote(&format!("{r:?}")))),
                    opt(n.label.map(|l| quote(&l))),
                    r.left / s,
                    r.top / s,
                    r.width() / s,
                    r.height() / s,
                    opt(n.fill.map(|c| quote(&hex(c)))),
                    opt(n.text.map(|t| quote(&t))),
                    opt(n.checked.map(|c| c.to_string())),
                    n.state,
                    n.focused,
                )
            })
            .collect();
        format!(
            "{{\"scale\":{s},\"size\":[{},{}],\"nodes\":[\n{}\n]}}\n",
            w as f32 / s,
            h as f32 / s,
            nodes.join(",\n")
        )
    }
}

fn hex(c: Rgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b), b(c.a))
}

#[derive(Debug, Default)]
struct Args {
    snap: Option<PathBuf>,
    tree: Option<PathBuf>,
    script: Option<PathBuf>,
    scale: Option<f32>,
    at: Option<Duration>,
    inactive: bool,
}

impl Args {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut out = Self::default();
        while let Some(flag) = args.next() {
            let mut value = || args.next().with_context(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--snap" => out.snap = Some(value()?.into()),
                "--tree" => out.tree = Some(value()?.into()),
                "--script" => out.script = Some(value()?.into()),
                "--scale" => out.scale = Some(value()?.parse()?),
                "--at" => out.at = Some(Duration::from_secs_f64(value()?.parse()?)),
                "--inactive" => out.inactive = true,
                other => bail!("unknown flag {other} (--snap, --tree, --script, --scale, --at, --inactive)"),
            }
        }
        Ok(out)
    }
}

/// An app window's opaque backing: the theme's surface over black or white.
fn backing(theme: &Theme) -> Rgba {
    let c = theme.color.surface;
    let under = if theme.dark() { 0.0 } else { 1.0 };
    let mix = |v: f32| v * c.a + under * (1.0 - c.a);
    Rgba::new(mix(c.r), mix(c.g), mix(c.b), 1.0)
}

fn device((w, h): (f32, f32), s: f32) -> (u32, u32) {
    ((w * s).round() as u32, (h * s).round() as u32)
}

/// Runs `app` until its window closes, or once headless when given agent flags (see the
/// module docs).
pub fn run<S: 'static, V: Fn(&S) -> El<S> + 'static>(app: App<S, V>) -> Result<()> {
    let args = Args::parse(std::env::args().skip(1))?;
    if args.snap.is_some() || args.tree.is_some() || args.script.is_some() {
        return headless(app, &args);
    }
    window(app, &args)
}

fn headless<S: 'static, V: Fn(&S) -> El<S> + 'static>(app: App<S, V>, args: &Args) -> Result<()> {
    let s = args.scale.unwrap_or(1.0);
    let opaque = matches!(app.kind, Kind::App { .. }).then(|| backing(&app.theme));
    let mut rt = Runtime::new(app.state, app.view, app.theme, device(app.size, s), s)?;
    rt.set_background(opaque);
    match &args.script {
        Some(path) => {
            let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
            rt.play(&Script::parse(&text)?, args.at);
        }
        None => {
            rt.frame(args.at.unwrap_or_default());
        }
    }
    rt.frame(Duration::ZERO);
    if let Some(path) = &args.snap {
        std::fs::write(path, super::png::encode(&rt.snap()?)).with_context(|| path.display().to_string())?;
    }
    match &args.tree {
        Some(path) => std::fs::write(path, rt.tree_json()).with_context(|| path.display().to_string())?,
        None if args.snap.is_none() => print!("{}", rt.tree_json()),
        None => {}
    }
    Ok(())
}

fn window<S: 'static, V: Fn(&S) -> El<S> + 'static>(app: App<S, V>, args: &Args) -> Result<()> {
    let mut surface = Surface::new(app.kind, app.title, Role::Window)?;
    let is_app = matches!(app.kind, Kind::App { .. });
    let s = args.scale.unwrap_or(if is_app { surface.scale() } else { 1.0 });
    let (w, h) = device(app.size, s);
    let (origin, size) = if is_app {
        let (x, y) = centred((w, h));
        surface.place((x, y, w as i32, h as i32));
        ((x, y), surface.client_size())
    } else {
        (centred((w, h)), (w, h))
    };
    let opaque = is_app.then(|| backing(&app.theme));
    let mut rt = Runtime::new(app.state, app.view, app.theme, size, s)?;
    rt.set_background(opaque);
    rt.frame(Duration::ZERO);
    if let Some(backing) = opaque {
        surface.set_background(backing);
    }
    let chrome = |surface: &Surface, rt: &Runtime<S>| {
        if is_app {
            let band = Rect::new(0.0, 0.0, rt.size().0 as f32, (32.0 * rt.scale()).round());
            surface.set_chrome(&[(band, Chrome::Caption)]);
        }
    };
    chrome(&surface, &rt);
    rt.present(&mut surface, origin)?;
    match (is_app, args.inactive) {
        (true, false) => surface.show_active(false),
        _ => surface.show(),
    }
    let started = Instant::now();
    // Events our own calls queued (showing, presenting) wait here instead of in the queue.
    let mut held = next_event();
    loop {
        if held.is_none() {
            wait();
        }
        for (id, event) in held.take().into_iter().chain(std::iter::from_fn(next_event)) {
            if id != surface.id() {
                continue;
            }
            match event {
                Event::Close => return Ok(()),
                Event::Resized(w, h) => rt.resize((w, h)),
                Event::DpiChanged(dpi) => rt.set_scale(dpi as f32 / 96.0),
                Event::Key(0x09) => rt.tab(shift_down()),
                event => rt.handle(&event),
            }
        }
        if rt.frame(started.elapsed().saturating_sub(rt.now())) {
            chrome(&surface, &rt);
            rt.present(&mut surface, origin)?;
        }
        held = next_event();
    }
}

/// The top-left that centres `size` on the primary screen.
fn centred((w, h): (u32, u32)) -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    // SAFETY: plain metric queries.
    let (sw, sh) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    ((sw - w as i32) / 2, (sh - h as i32) / 2)
}

fn shift_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_SHIFT};
    // SAFETY: a query of this thread's key state.
    unsafe { GetKeyState(i32::from(VK_SHIFT.0)) < 0 }
}

/// Sleeps until a window message arrives, then dispatches every message waiting.
fn wait() {
    use windows::Win32::System::Threading::INFINITE;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx,
        PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage,
    };
    // SAFETY: a standard message pump on this thread, with no handles to outlive.
    unsafe {
        MsgWaitForMultipleObjectsEx(None, INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
