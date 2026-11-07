//! Hosting a view: [`run`] opens a surface and drives a [`Runtime`] from its events, asleep
//! in the message wait whenever nothing happens. Display-rate frames
//! ([`Surface::animation_frames`]) run only while something animates, and stop when it
//! settles. The same binary is the agent tooling, headless on a virtual clock:
//!
//! ```text
//! app --snap out.png [--scale 2] [--at 1.5]   render the frame at T seconds
//! app --tree out.json                          dump every element (key, path, role, rect...)
//! app --script s.json [--snap ..] [--tree ..]  replay timed input first
//! app --inactive                               a window that never takes the focus
//! app --reduced                                reduced motion, whatever the system says
//! ```
//!
//! Scripts and dumps use design pixels. A script is a JSON array of steps, each with an `at`
//! time in seconds and one input: `{"at": 0.2, "move": [x, y]}`, `"down"`, `"up"`,
//! `{"right": [x, y]}` (the secondary button, for context menus), `{"click": "key"}` (the
//! centre of a keyed element), `{"key": "Tab"}` (also `Shift+Tab`,
//! `Enter`, `Space`, `Escape`, arrows `Left` `Right` `Up` `Down`, `PageUp`, `PageDown`, `Home`,
//! `End`, `Menu`, or a virtual-key number), `{"wheel": [x, y, delta]}` (120 a notch, positive up).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use accesskit::Role;
use anyhow::{Context, Result, bail};

use super::json::{Json, parse, quote};
use super::{El, Runtime};
use crate::platform::current::host::{reduced_motion, screen_size, shift_down, wait};
use crate::{AnimationFrames, Chrome, Event, Kind, Rect, Rgba, Surface, Theme, next_event};

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
    /// The wheel at a point: 120 a notch, positive up.
    Wheel(f32, f32, i16),
    /// The secondary button pressed and released at a point (a context menu).
    Right(f32, f32),
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
            } else if let Some((x, y)) = point("right") {
                Input::Right(x, y)
            } else if let Some(key) = step.get("click").and_then(Json::str) {
                Input::Click(key.to_owned())
            } else if let Some(Json::Arr(w)) = step.get("wheel")
                && let [x, y, delta] = w.as_slice()
                && let (Some(x), Some(y), Some(delta)) = (x.num(), y.num(), delta.num())
            {
                Input::Wheel(x as f32, y as f32, delta as i16)
            } else if let Some(key) = step.get("key") {
                key_of(key).with_context(|| format!("step {i}: unknown key {key:?}"))?
            } else {
                bail!("step {i}: no input (move, down, up, right, click, key or wheel)");
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
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "End" => 0x23,
        "Home" => 0x24,
        "Left" => 0x25,
        "Up" => 0x26,
        "Right" => 0x27,
        "Down" => 0x28,
        "Menu" => 0x5D,
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
                Input::Wheel(x, y, delta) => self.wheel_at(x * s, y * s, *delta),
                Input::Right(x, y) => self.right_click(x * s, y * s),
            }
            self.frame(Duration::ZERO);
        }
        if let Some(until) = until {
            self.frame((start + until).saturating_sub(self.now()));
        }
    }

    /// The `--tree` dump: every element as shown now, in design pixels, with what assistive
    /// technology reads (`range` is `[min, max, value]`; `scroll` a scroll view's offset).
    pub fn tree_json(&self) -> String {
        let s = self.scale();
        let (w, h) = self.size();
        let opt = |v: Option<String>| v.unwrap_or_else(|| "null".into());
        let text = |v: Option<std::sync::Arc<str>>| opt(v.map(|t| quote(&t)));
        let nodes: Vec<String> = self
            .tree()
            .into_iter()
            .map(|n| {
                let r = n.rect;
                format!(
                    "{{\"key\":{},\"path\":{},\"role\":{},\"label\":{},\"rect\":[{},{},{},{}],\"fill\":{},\"text\":{},\"checked\":{},\"state\":\"{:?}\",\"focused\":{},\"range\":{},\"value\":{},\"description\":{},\"selected\":{},\"expanded\":{},\"scroll\":{}}}",
                    opt(n.key.map(|k| quote(&k))),
                    quote(if n.path.is_empty() { "/" } else { &n.path }),
                    opt(n.role.map(|r| quote(&format!("{r:?}")))),
                    opt(n.label.map(|l| quote(&l))),
                    r.left / s,
                    r.top / s,
                    r.width() / s,
                    r.height() / s,
                    opt(n.fill.map(|c| quote(&hex(c)))),
                    text(n.text),
                    opt(n.checked.map(|c| c.to_string())),
                    n.state,
                    n.focused,
                    opt(n.range.map(|(min, max, value)| format!("[{min},{max},{value}]"))),
                    text(n.value),
                    text(n.description),
                    opt(n.selected.map(|v| v.to_string())),
                    opt(n.expanded.map(|v| v.to_string())),
                    opt(n.scroll.map(|v| (v / s).to_string())),
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
    reduced: bool,
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
                "--reduced" => out.reduced = true,
                other => bail!("unknown flag {other} (--snap, --tree, --script, --scale, --at, --inactive, --reduced)"),
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
    rt.set_reduced_motion(args.reduced);
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
    rt.set_reduced_motion(args.reduced || reduced_motion());
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
    // Display-rate frames, only while something animates: dropping them stops the producer.
    let mut frames: Option<(AnimationFrames, Instant)> = None;
    // One wake for the runtime's next timer (a tooltip's delay, a toast), none once it has none.
    let mut waker = Waker::default();
    // The client area's top-left on screen (pointer events correct it), for popup surfaces,
    // and the screen area around the window last handed to the runtime.
    #[cfg(windows)]
    let (mut client, mut popups, mut around) = (origin, Popups::default(), None);
    // Events our own calls queued (showing, presenting) wait here instead of in the queue.
    let mut held = next_event();
    loop {
        if held.is_none() {
            wait();
        }
        for (id, event) in held.take().into_iter().chain(std::iter::from_fn(next_event)) {
            if id != surface.id() {
                #[cfg(windows)]
                if let Some(event) = popups.input(id, event) {
                    rt.handle(&event);
                }
                continue;
            }
            #[cfg(windows)]
            if let Event::Move(p) | Event::Press(p, _) | Event::Release(p, _) = event {
                client = (p.screen.0 - p.at.x as i32, p.screen.1 - p.at.y as i32);
            }
            match event {
                Event::Close => return Ok(()),
                Event::Frame => {}
                Event::Resized(w, h) => rt.resize((w, h)),
                Event::DpiChanged(dpi) => rt.set_scale(dpi as f32 / 96.0),
                Event::Key(0x09) => rt.tab(shift_down()),
                Event::SettingChanged => rt.set_reduced_motion(args.reduced || reduced_motion()),
                // A slider's increment or a new value comes with its invoke.
                Event::Invoke(hit) => match surface.take_accessibility_action(hit) {
                    Some(action) => {
                        rt.accessibility_action(hit, &action);
                    }
                    None => rt.handle(&event),
                },
                event => rt.handle(&event),
            }
        }
        #[cfg(windows)]
        if around != Some((client, rt.size())) {
            around = Some((client, rt.size()));
            rt.set_screen(Some(screen_around(client, rt.size())));
        }
        if rt.frame(started.elapsed().saturating_sub(rt.now())) {
            chrome(&surface, &rt);
            rt.present(&mut surface, origin)?;
            #[cfg(windows)]
            popups.sync(&mut rt, client)?;
        }
        // Where the host has no one-shot wake, frames stand in while a timer waits.
        let waits = !Waker::ONE_SHOT && rt.wake_at().is_some();
        if !rt.animating() && !waits {
            frames = None;
        } else if frames.as_ref().is_none_or(|(_, since)| since.elapsed() >= FRAMES) {
            frames = Some((surface.animation_frames(FRAMES), Instant::now()));
        }
        waker.arm(&surface, rt.wake_at().map(|at| started + at));
        held = next_event();
    }
}

/// How long one frame producer runs before it is renewed (if anything still moves).
const FRAMES: Duration = Duration::from_secs(10);

/// Wakes the UI thread once at the runtime's next timer: a sleeping thread posts one message
/// to the window, unless a newer deadline (or none) replaced it. Nothing stays armed once the
/// runtime has no timer.
#[derive(Default)]
struct Waker {
    armed: Option<Instant>,
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Waker {
    /// Whether this platform wakes the message wait from another thread (else frames do).
    const ONE_SHOT: bool = cfg!(windows);

    fn arm(&mut self, surface: &Surface, at: Option<Instant>) {
        use std::sync::atomic::Ordering;
        if at == self.armed {
            return;
        }
        self.armed = at;
        let mine = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        #[cfg(windows)]
        if let Some(at) = at {
            let generation = self.generation.clone();
            let hwnd = surface.hwnd().0 as isize;
            let _ = std::thread::Builder::new().name("ui-wake".into()).spawn(move || {
                std::thread::sleep(at.saturating_duration_since(Instant::now()));
                if generation.load(Ordering::Relaxed) == mine {
                    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_NULL};
                    // SAFETY: posting is safe from any thread; a destroyed window refuses it.
                    let _ = unsafe { PostMessageW(Some(HWND(hwnd as _)), WM_NULL, WPARAM(0), LPARAM(0)) };
                }
            });
        }
        #[cfg(not(windows))]
        let _ = (surface, mine);
    }
}

/// The work area of the monitor under the middle of the window, in the window's pixels
/// (where overlays may escape to).
#[cfg(windows)]
fn screen_around(client: (i32, i32), size: (u32, u32)) -> Rect {
    let (cx, cy) = (client.0 + size.0 as i32 / 2, client.1 + size.1 as i32 / 2);
    let (l, t, r, b) = crate::screen_at(cx, cy).work;
    Rect::new((l - client.0) as f32, (t - client.1) as f32, (r - l) as f32, (b - t) as f32)
}

/// Popup surfaces showing the overlays that escaped the window ([`Runtime::popups`]): made
/// when one opens (or taken from a pool), shown without activation so keys stay with the
/// window, and hidden again (their bitmaps released) when it closes.
#[cfg(windows)]
#[derive(Default)]
struct Popups {
    open: Vec<(u64, Surface, Rect)>,
    pool: Vec<Surface>,
}

#[cfg(windows)]
impl Popups {
    /// A popup surface's event as the runtime takes it: pointers in the window's pixels.
    /// The pointer leaving a popup is the window's business (it gets the pointer next).
    fn input(&self, id: crate::SurfaceId, event: Event) -> Option<Event> {
        let (_, _, rect) = self.open.iter().find(|(_, s, _)| s.id() == id)?;
        let shift = |mut p: crate::Pointer| {
            p.at = crate::Point::new(p.at.x + rect.left, p.at.y + rect.top);
            p
        };
        Some(match event {
            Event::Move(p) => Event::Move(shift(p)),
            Event::Press(p, b) => Event::Press(shift(p), b),
            Event::Release(p, b) => Event::Release(shift(p), b),
            Event::DoubleClick(p, b) => Event::DoubleClick(shift(p), b),
            Event::Wheel(p, d) => Event::Wheel(shift(p), d),
            Event::Invoke(_) | Event::Focus(_) => event,
            _ => return None,
        })
    }

    fn sync<S>(&mut self, rt: &mut Runtime<S>, client: (i32, i32)) -> Result<()> {
        let want = rt.popups().to_vec();
        let mut i = 0;
        while i < self.open.len() {
            if want.iter().any(|p| p.id == self.open[i].0) {
                i += 1;
                continue;
            }
            let (_, mut surface, _) = self.open.swap_remove(i);
            surface.hide();
            surface.release();
            surface.discard_events();
            self.pool.push(surface);
        }
        for popup in want {
            let k = match self.open.iter().position(|(id, ..)| *id == popup.id) {
                Some(k) => k,
                None => {
                    let surface = match self.pool.pop() {
                        Some(surface) => surface,
                        None => Surface::new(Kind::Popup { activate: false }, "Overlay", Role::Menu)?,
                    };
                    self.open.push((popup.id, surface, popup.rect));
                    self.open.len() - 1
                }
            };
            let r = popup.rect;
            let at = (client.0 + r.left as i32, client.1 + r.top as i32);
            let (_, surface, rect) = &mut self.open[k];
            *rect = r;
            surface.present(rt.ui(), at, (r.width() as u32, r.height() as u32), popup.list)?;
            surface.show();
        }
        Ok(())
    }
}

/// The top-left that centres `size` on the primary screen.
fn centred((w, h): (u32, u32)) -> (i32, i32) {
    let (sw, sh) = screen_size();
    ((sw - w as i32) / 2, (sh - h as i32) / 2)
}
