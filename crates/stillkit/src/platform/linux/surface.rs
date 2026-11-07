use crate::{DisplayList, HitId, Point, Ui};
use anyhow::{Context, Result, bail};
/// What kind of window a surface is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Parentless popup fallback (xdg toplevel). Use `Surface::popup` with a parent
    /// for a real xdg popup. Activation always belongs to the compositor.
    Popup { activate: bool },
    /// Layer-shell top-layer surface; unavailable on GNOME.
    Bar,
    /// Layer-shell background surface; `parent` is retained for source compatibility.
    DesktopChild { parent: usize },
    /// A normal xdg toplevel with client-side chrome. Minimum size is logical pixels.
    App { min: (u32, u32) },
}

/// Identifies a surface in [`Event`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(isize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// Where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// In the surface's pixels.
    pub at: Point,
    /// In screen pixels.
    pub screen: (i32, i32),
}

/// Input for a surface, taken from [`next_event`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// A temporary animation requested another frame; no timer remains once it settles.
    Frame,
    Move(Pointer),
    Leave,
    /// Positive delta scrolls up; position uses the same coordinates as other pointer events.
    Wheel(Pointer, i16),
    CaptureLost,
    Press(Pointer, Button),
    Release(Pointer, Button),
    DoubleClick(Pointer, Button),
    /// A key went down (a virtual-key code).
    Key(u16),
    /// A desktop surface activated; Explorer may finish reordering its band afterwards.
    Activated,
    /// The surface lost the activation (another window was clicked or focused).
    Deactivated,
    /// Assistive technology invoked a hit item.
    Invoke(HitId),
    /// Assistive technology moved the focus to a hit item.
    Focus(HitId),
    DpiChanged(u32),
    VisibilityChanged,
    Cancelled,
    /// Windowless RichEdit changed text, selection or caret.
    TextChanged,
    /// Enter in a single-line windowless field.
    TextSubmit,
    /// An app window's client area is now this many pixels.
    Resized(u32, u32),
    /// The user asked an app window to close (its close button, Alt+F4, the taskbar).
    Close,
    /// Windows' settings or theme changed (light or dark, contrast, text size).
    SettingChanged,
}

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    os::fd::{AsFd, FromRawFd},
    rc::Rc,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry,
        wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
};
pub use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;
use wayland_protocols::xdg::shell::client::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1 as layer_shell, zwlr_layer_surface_v1 as layer_surface,
};
struct Buffers {
    map: memmap2::MmapMut,
    buffers: Vec<wl_buffer::WlBuffer>,
    busy: [bool; 2],
    size: (u32, u32),
}
impl Drop for Buffers {
    fn drop(&mut self) {
        for b in &self.buffers {
            b.destroy();
        }
    }
}
enum Shell {
    Top(xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel),
    Popup(xdg_surface::XdgSurface, xdg_popup::XdgPopup),
    Layer(layer_surface::ZwlrLayerSurfaceV1),
}
struct Native {
    surface: wl_surface::WlSurface,
    shell: Shell,
    configured: bool,
    buffers: Option<Buffers>,
    scale: u32,
    animation: Option<(Rc<Cell<bool>>, Instant)>,
    callback: bool,
    chrome: Vec<(crate::Rect, super::app_window::Chrome)>,
    maximized: bool,
}
impl Drop for Native {
    fn drop(&mut self) {
        match &self.shell {
            Shell::Top(s, t) => {
                t.destroy();
                s.destroy();
            }
            Shell::Popup(s, p) => {
                p.destroy();
                s.destroy();
            }
            Shell::Layer(l) => l.destroy(),
        }
        self.surface.destroy();
    }
}
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm: Option<xdg_wm_base::XdgWmBase>,
    layer: Option<layer_shell::ZwlrLayerShellV1>,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    xkb: Option<super::keyboard::Keyboard>,
    windows: HashMap<SurfaceId, Native>,
    events: VecDeque<(SurfaceId, Event)>,
    next: isize,
    scale: u32,
    screen_size: (i32, i32),
    at: Point,
    pointer_focus: Option<SurfaceId>,
    key_focus: Option<SurfaceId>,
    serial: u32,
}
struct Runtime {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
}
thread_local! { static RUNTIME:RefCell<Option<Runtime>>=const{RefCell::new(None)}; }
fn runtime<T>(f: impl FnOnce(&mut Runtime) -> Result<T>) -> Result<T> {
    RUNTIME.with_borrow_mut(|slot| {
        if slot.is_none() {
            let conn = Connection::connect_to_env()?;
            let mut queue = conn.new_event_queue();
            conn.display().get_registry(&queue.handle(), ());
            let mut state = State {
                compositor: None,
                shm: None,
                wm: None,
                layer: None,
                seat: None,
                pointer: None,
                keyboard: None,
                xkb: None,
                windows: HashMap::new(),
                events: VecDeque::new(),
                next: 1,
                scale: 1,
                screen_size: (1920, 1080),
                at: Point::default(),
                pointer_focus: None,
                key_focus: None,
                serial: 0,
            };
            queue.roundtrip(&mut state)?;
            queue.roundtrip(&mut state)?;
            *slot = Some(Runtime { conn, queue, state });
        }
        f(slot.as_mut().unwrap())
    })
}
/// Keeps compositor frame callbacks armed until dropped or the duration expires.
pub struct AnimationFrames(Rc<Cell<bool>>);
impl Drop for AnimationFrames {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
fn frame(n: &mut Native, q: &QueueHandle<State>, id: SurfaceId) {
    if !n.callback
        && n.animation
            .as_ref()
            .is_some_and(|(a, end)| a.get() && Instant::now() < *end)
    {
        n.surface.frame(q, id);
        n.callback = true;
    }
}
/// Blocks for native input when the queue is empty; idle does no work.
pub fn next_event() -> Option<(SurfaceId, Event)> {
    runtime(|r| {
        loop {
            r.queue.dispatch_pending(&mut r.state)?;
            if let Some(e) = r.state.events.pop_front() {
                return Ok(Some(e));
            }
            r.queue.blocking_dispatch(&mut r.state)?;
        }
    })
    .unwrap_or_else(|e| {
        log::error!("Wayland dispatch: {e}");
        None
    })
}
pub(crate) fn queue_for(id: SurfaceId, event: Event) {
    queue_event(id.0 as usize, event);
}
pub fn queue_event(id: usize, event: Event) {
    let _ = runtime(|r| {
        r.state.events.push_back((SurfaceId(id as isize), event));
        Ok(())
    });
}
/// A Wayland surface with retained display list and CPU damage bitmap.
pub struct Surface {
    id: SurfaceId,
    alpha: u8,
    shown: DisplayList,
    canvas: Option<super::render::Canvas>,
    origin: (i32, i32),
}
impl Surface {
    pub fn new(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
        Self::create(kind, name, role, None)
    }
    /// Creates a parent-relative xdg popup. Coordinates are logical pixels in the parent.
    pub fn popup(parent: &Surface, name: &str, at: (i32, i32)) -> Result<Self> {
        let s = Self::create(
            Kind::Popup { activate: false },
            name,
            accesskit::Role::Window,
            Some((parent.id, at)),
        )?;
        Ok(s)
    }
    fn create(
        kind: Kind,
        name: &str,
        _: accesskit::Role,
        parent: Option<(SurfaceId, (i32, i32))>,
    ) -> Result<Self> {
        runtime(|r| {
            let q = r.queue.handle();
            let id = SurfaceId(r.state.next);
            r.state.next += 1;
            if matches!(kind, Kind::Bar | Kind::DesktopChild { .. }) && r.state.layer.is_none() {
                bail!("compositor does not advertise wlr-layer-shell");
            }
            let surface = r
                .state
                .compositor
                .as_ref()
                .context("wl_compositor unavailable")?
                .create_surface(&q, id);
            let shell = if matches!(kind, Kind::Bar | Kind::DesktopChild { .. }) {
                let l = r.state.layer.as_ref().unwrap().get_layer_surface(
                    &surface,
                    None,
                    if matches!(kind, Kind::Bar) {
                        layer_shell::Layer::Top
                    } else {
                        layer_shell::Layer::Background
                    },
                    "stillkit".into(),
                    &q,
                    id,
                );
                l.set_size(640, 48);
                l.set_anchor(
                    layer_surface::Anchor::Top
                        | layer_surface::Anchor::Left
                        | layer_surface::Anchor::Right,
                );
                l.set_keyboard_interactivity(layer_surface::KeyboardInteractivity::None);
                Shell::Layer(l)
            } else {
                let s = r
                    .state
                    .wm
                    .as_ref()
                    .context("xdg-shell unavailable")?
                    .get_xdg_surface(&surface, &q, id);
                if let Some((pid, at)) = parent {
                    let parent = r.state.windows.get(&pid).context("popup parent gone")?;
                    let ps = match &parent.shell {
                        Shell::Top(s, _) | Shell::Popup(s, _) => s,
                        Shell::Layer(_) => bail!("layer popup parent unsupported"),
                    };
                    let pos = r.state.wm.as_ref().unwrap().create_positioner(&q, ());
                    pos.set_size(320, 240);
                    pos.set_anchor_rect(at.0, at.1, 1, 1);
                    pos.set_anchor(xdg_positioner::Anchor::BottomRight);
                    pos.set_gravity(xdg_positioner::Gravity::BottomRight);
                    let p = s.get_popup(Some(ps), &pos, &q, id);
                    pos.destroy();
                    Shell::Popup(s, p)
                } else {
                    let t = s.get_toplevel(&q, id);
                    t.set_title(name.into());
                    t.set_app_id("org.stillkit.app".into());
                    if let Kind::App { min } = kind {
                        t.set_min_size(min.0 as i32, min.1 as i32);
                    }
                    Shell::Top(s, t)
                }
            };
            surface.commit();
            r.state.windows.insert(
                id,
                Native {
                    surface,
                    shell,
                    configured: false,
                    buffers: None,
                    scale: r.state.scale,
                    animation: None,
                    callback: false,
                    chrome: Vec::new(),
                    maximized: false,
                },
            );
            while !r.state.windows[&id].configured {
                r.queue.blocking_dispatch(&mut r.state)?;
            }
            Ok(Self {
                id,
                alpha: 255,
                shown: DisplayList::default(),
                canvas: None,
                origin: (0, 0),
            })
        })
    }
    pub fn new_deferred_accessibility(k: Kind, n: &str, r: accesskit::Role) -> Result<Self> {
        Self::new(k, n, r)
    }
    pub fn present(
        &mut self,
        ui: &mut Ui,
        origin: (i32, i32),
        size: (u32, u32),
        list: DisplayList,
    ) -> Result<bool> {
        let resized = self.canvas.as_ref().is_none_or(|c| c.size() != size);
        let damage = if resized {
            Some(crate::Rect::new(0., 0., size.0 as f32, size.1 as f32))
        } else {
            list.damage(&self.shown)
        };
        self.origin = origin;
        if let Some(area) = damage.and_then(|d| d.pixels(size.0, size.1)) {
            if resized {
                self.canvas = Some(super::render::Canvas::new(ui, size.0, size.1)?);
            }
            let c = self.canvas.as_ref().unwrap();
            c.draw(&mut ui.text, &list, area)?;
            self.send(&c.image(), area)?;
            self.shown = list;
            Ok(true)
        } else {
            self.shown = list;
            Ok(false)
        }
    }
    fn send(&self, image: &crate::Image, area: crate::PixelRect) -> Result<()> {
        runtime(|r| {
            let q = r.queue.handle();
            let n = r
                .state
                .windows
                .get_mut(&self.id)
                .context("surface released")?;
            anyhow::ensure!(
                image.width > 0
                    && image.height > 0
                    && image.width <= 32768
                    && image.height <= 32768
                    && image.pixels.len() == image.width as usize * image.height as usize * 4,
                "invalid presentation bitmap"
            );
            let size = (image.width, image.height);
            let bytes = image.pixels.len();
            if n.buffers.as_ref().is_none_or(|b| b.size != size) {
                // SAFETY: memfd_create returns an owned descriptor, checked before File takes it.
                let fd = unsafe { libc::memfd_create(c"stillkit-shm".as_ptr(), libc::MFD_CLOEXEC) };
                ensure_fd(fd)?;
                // SAFETY: fd is newly created and ownership is transferred exactly once.
                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                file.set_len((bytes * 2) as u64)?;
                // SAFETY: this owned memfd stays sized for both buffers; no file truncation occurs.
                let map = unsafe { memmap2::MmapOptions::new().map_mut(&file)? };
                let pool = r
                    .state
                    .shm
                    .as_ref()
                    .context("wl_shm unavailable")?
                    .create_pool(file.as_fd(), (bytes * 2) as i32, &q, ());
                let buffers = (0..2)
                    .map(|i| {
                        pool.create_buffer(
                            (i * bytes) as i32,
                            size.0 as i32,
                            size.1 as i32,
                            (size.0 * 4) as i32,
                            wl_shm::Format::Argb8888,
                            &q,
                            (self.id, i),
                        )
                    })
                    .collect();
                pool.destroy();
                n.buffers = Some(Buffers {
                    map,
                    buffers,
                    busy: [false; 2],
                    size,
                });
            }
            // Wait only for the compositor to release a buffer; never overwrite an attached one.
            while r.state.windows[&self.id]
                .buffers
                .as_ref()
                .unwrap()
                .busy
                .iter()
                .all(|v| *v)
            {
                r.queue.blocking_dispatch(&mut r.state)?;
            }
            let n = r.state.windows.get_mut(&self.id).unwrap();
            let b = n.buffers.as_mut().unwrap();
            let i = b.busy.iter().position(|v| !*v).unwrap();
            if self.alpha == 255 {
                b.map[i * bytes..(i + 1) * bytes].copy_from_slice(&image.pixels);
            } else {
                for (dst, src) in b.map[i * bytes..(i + 1) * bytes]
                    .iter_mut()
                    .zip(&image.pixels)
                {
                    *dst = (*src as u16 * self.alpha as u16 / 255) as u8;
                }
            }
            b.busy[i] = true;
            n.surface.set_buffer_scale(n.scale as i32);
            n.surface.attach(Some(&b.buffers[i]), 0, 0);
            n.surface.damage_buffer(
                area.x as i32,
                area.y as i32,
                area.width as i32,
                area.height as i32,
            );
            frame(n, &q, self.id);
            n.surface.commit();
            r.conn.flush()?;
            Ok(())
        })
    }
    pub fn shown(&self) -> &DisplayList {
        &self.shown
    }
    pub fn id(&self) -> SurfaceId {
        self.id
    }
    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }
    pub fn scale(&self) -> u32 {
        runtime(|r| Ok(r.state.windows[&self.id].scale)).unwrap_or(1)
    }
    pub fn animation_frames(&self, duration: Duration) -> AnimationFrames {
        let a = Rc::new(Cell::new(true));
        let _ = runtime(|r| {
            let n = r
                .state
                .windows
                .get_mut(&self.id)
                .context("surface released")?;
            n.animation = Some((a.clone(), Instant::now() + duration));
            frame(n, &r.queue.handle(), self.id);
            n.surface.commit();
            r.conn.flush()?;
            Ok(())
        });
        AnimationFrames(a)
    }
    pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
        if self.alpha != alpha {
            self.alpha = alpha;
            if let Some(c) = &self.canvas {
                let image = c.image();
                self.send(&image, crate::PixelRect::full(image.width, image.height))?;
            }
        }
        Ok(())
    }
    pub fn move_to(&mut self, _: (i32, i32)) -> Result<()> {
        bail!("Wayland does not permit absolute window positioning")
    }
    pub fn set_glass(&self, _: Option<crate::Glass>) {}
    pub fn set_glass_style(&self, _: crate::glass::GlassStyle) {}
    /// Remaps a hidden surface with its retained CPU bitmap.
    pub fn show(&self) {
        let result = runtime(|r| {
            if r.state.windows.get(&self.id).is_some_and(|n| !n.configured) {
                r.state.windows[&self.id].surface.commit();
                r.conn.flush()?;
                while !r.state.windows[&self.id].configured {
                    r.queue.blocking_dispatch(&mut r.state)?;
                }
                Ok(true)
            } else {
                Ok(false)
            }
        });
        match result {
            Ok(true) => {
                if let Some(c) = &self.canvas {
                    let image = c.image();
                    if let Err(e) =
                        self.send(&image, crate::PixelRect::full(image.width, image.height))
                    {
                        log::error!("Wayland show: {e}");
                    }
                }
            }
            Err(e) => log::error!("Wayland show: {e}"),
            _ => {}
        }
    }
    pub fn hide(&self) {
        let _ = runtime(|r| {
            if let Some(n) = r.state.windows.get_mut(&self.id) {
                n.configured = false;
                n.surface.attach(None, 0, 0);
                n.surface.commit();
            }
            r.conn.flush()?;
            Ok(())
        });
    }
    pub fn release(&mut self) {
        let _ = runtime(|r| {
            r.state.windows.remove(&self.id);
            r.conn.flush()?;
            Ok(())
        });
    }
    pub fn discard_events(&self) {
        let _ = runtime(|r| {
            r.state.events.retain(|(id, _)| *id != self.id);
            Ok(())
        });
    }
    pub fn take_focus(&self) -> &'static str {
        "Wayland focus is compositor controlled"
    }
    pub fn set_chrome(&self, parts: &[(crate::Rect, super::app_window::Chrome)]) {
        let _ = runtime(|r| {
            if let Some(n) = r.state.windows.get_mut(&self.id) {
                n.chrome = parts.to_vec();
            }
            Ok(())
        });
    }
    pub fn minimize(&self) -> Result<()> {
        runtime(|r| {
            if let Shell::Top(_, t) = &r.state.windows[&self.id].shell {
                t.set_minimized();
            }
            r.conn.flush()?;
            Ok(())
        })
    }
    pub fn toggle_maximize(&self) -> Result<()> {
        runtime(|r| {
            let n = r
                .state
                .windows
                .get_mut(&self.id)
                .context("surface released")?;
            if let Shell::Top(_, t) = &n.shell {
                if n.maximized {
                    t.unset_maximized();
                } else {
                    t.set_maximized();
                }
            }
            r.conn.flush()?;
            Ok(())
        })
    }
    /// Starts compositor-managed dragging after a real pointer press.
    pub fn start_move(&self) -> Result<()> {
        runtime(|r| {
            if let (Some(seat), Shell::Top(_, top)) =
                (&r.state.seat, &r.state.windows[&self.id].shell)
            {
                top._move(seat, r.state.serial);
                r.conn.flush()?;
            }
            Ok(())
        })
    }
    pub fn start_resize(&self, edge: xdg_toplevel::ResizeEdge) -> Result<()> {
        runtime(|r| {
            if let (Some(seat), Shell::Top(_, top)) =
                (&r.state.seat, &r.state.windows[&self.id].shell)
            {
                top.resize(seat, r.state.serial, edge);
                r.conn.flush()?;
            }
            Ok(())
        })
    }
}
fn ensure_fd(fd: i32) -> Result<()> {
    if fd < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        self.release();
    }
}
impl crate::platform::Window for Surface {
    type Bitmap = crate::Image;
    fn new(k: Kind, n: &str, r: accesskit::Role) -> Result<Self> {
        Self::new(k, n, r)
    }
    fn present_bitmap(
        &mut self,
        _: (i32, i32),
        bitmap: Option<(&crate::Image, crate::PixelRect, bool)>,
    ) -> Result<()> {
        if let Some((image, damage, _)) = bitmap {
            self.send(image, damage)?;
        }
        Ok(())
    }
    fn next_event() -> Option<(SurfaceId, Event)> {
        next_event()
    }
}
impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        a: &mut Self,
        r: &wl_registry::WlRegistry,
        e: wl_registry::Event,
        _: &(),
        _: &Connection,
        q: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = e
        {
            match interface.as_str() {
                "wl_compositor" => a.compositor = Some(r.bind(name, version.min(6), q, ())),
                "wl_shm" => a.shm = Some(r.bind(name, 1, q, ())),
                "xdg_wm_base" => a.wm = Some(r.bind(name, version.min(6), q, ())),
                "zwlr_layer_shell_v1" => a.layer = Some(r.bind(name, version.min(4), q, ())),
                "wl_seat" => a.seat = Some(r.bind(name, version.min(8), q, ())),
                "wl_output" => {
                    let _: wl_output::WlOutput = r.bind(name, version.min(4), q, ());
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        a: &mut Self,
        _: &wl_output::WlOutput,
        e: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_output::Event::Scale { factor } => a.scale = a.scale.max(factor.max(1) as u32),
            wl_output::Event::Mode {
                flags: wayland_client::WEnum::Value(flags),
                width,
                height,
                ..
            } if flags.contains(wl_output::Mode::Current) => a.screen_size = (width, height),
            _ => {}
        }
    }
}
pub(crate) fn screen_info() -> ((i32, i32), u32) {
    runtime(|r| Ok((r.state.screen_size, r.state.scale))).unwrap_or(((1920, 1080), 1))
}
impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        p: &xdg_wm_base::XdgWmBase,
        e: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = e {
            p.pong(serial);
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, SurfaceId> for State {
    fn event(
        a: &mut Self,
        p: &xdg_surface::XdgSurface,
        e: xdg_surface::Event,
        id: &SurfaceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = e {
            p.ack_configure(serial);
            if let Some(n) = a.windows.get_mut(id) {
                n.configured = true;
            }
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel, SurfaceId> for State {
    fn event(
        a: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        e: xdg_toplevel::Event,
        id: &SurfaceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            xdg_toplevel::Event::Close => a.events.push_back((*id, Event::Close)),
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                if width > 0 && height > 0 {
                    let s = a.windows.get(id).map_or(1, |n| n.scale);
                    a.events
                        .push_back((*id, Event::Resized(width as u32 * s, height as u32 * s)));
                }
                if let Some(n) = a.windows.get_mut(id) {
                    n.maximized = states
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|v| u32::from_ne_bytes(*v) == 1);
                }
                let active = states
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|v| u32::from_ne_bytes(*v) == 4);
                log::debug!("Wayland activated={active}");
                a.events.push_back((
                    *id,
                    if active {
                        Event::Activated
                    } else {
                        Event::Deactivated
                    },
                ));
            }
            _ => {}
        }
    }
}
impl Dispatch<xdg_popup::XdgPopup, SurfaceId> for State {
    fn event(
        a: &mut Self,
        _: &xdg_popup::XdgPopup,
        e: xdg_popup::Event,
        id: &SurfaceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_popup::Event::PopupDone = e {
            a.events.push_back((*id, Event::Cancelled));
        }
    }
}
impl Dispatch<layer_surface::ZwlrLayerSurfaceV1, SurfaceId> for State {
    fn event(
        a: &mut Self,
        p: &layer_surface::ZwlrLayerSurfaceV1,
        e: layer_surface::Event,
        id: &SurfaceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            layer_surface::Event::Configure {
                serial,
                width,
                height,
            } => {
                p.ack_configure(serial);
                if let Some(n) = a.windows.get_mut(id) {
                    n.configured = true;
                    a.events
                        .push_back((*id, Event::Resized(width * n.scale, height * n.scale)));
                }
            }
            layer_surface::Event::Closed => a.events.push_back((*id, Event::Close)),
            _ => {}
        }
    }
}
impl Dispatch<wl_surface::WlSurface, SurfaceId> for State {
    fn event(
        a: &mut Self,
        _: &wl_surface::WlSurface,
        e: wl_surface::Event,
        id: &SurfaceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_surface::Event::PreferredBufferScale { factor } = e
            && let Some(n) = a.windows.get_mut(id)
        {
            let scale = factor.max(1) as u32;
            if scale != n.scale {
                n.scale = scale;
                a.events.push_back((*id, Event::DpiChanged(scale * 96)));
            }
        }
    }
}
impl Dispatch<wl_buffer::WlBuffer, (SurfaceId, usize)> for State {
    fn event(
        a: &mut Self,
        p: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        (id, i): &(SurfaceId, usize),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let Some(b) = a.windows.get_mut(id).and_then(|n| n.buffers.as_mut())
            && b.buffers[*i] == *p
        {
            b.busy[*i] = false;
        }
    }
}
impl Dispatch<wl_callback::WlCallback, SurfaceId> for State {
    fn event(
        a: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        id: &SurfaceId,
        _: &Connection,
        q: &QueueHandle<Self>,
    ) {
        if let Some(n) = a.windows.get_mut(id) {
            n.callback = false;
            if n.animation
                .as_ref()
                .is_some_and(|(active, end)| active.get() && Instant::now() < *end)
            {
                a.events.push_back((*id, Event::Frame));
                frame(n, q, *id);
                n.surface.commit();
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        a: &mut Self,
        p: &wl_seat::WlSeat,
        e: wl_seat::Event,
        _: &(),
        _: &Connection,
        q: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(c),
        } = e
        {
            if c.contains(wl_seat::Capability::Pointer) && a.pointer.is_none() {
                a.pointer = Some(p.get_pointer(q, ()));
            }
            if c.contains(wl_seat::Capability::Keyboard) && a.keyboard.is_none() {
                a.keyboard = Some(p.get_keyboard(q, ()));
            }
        }
    }
}
impl State {
    fn pointer_event(&mut self, e: impl FnOnce(Pointer) -> Event) {
        if let Some(id) = self.pointer_focus {
            let s = self.windows.get(&id).map_or(1, |n| n.scale) as f32;
            self.events.push_back((
                id,
                e(Pointer {
                    at: Point::new(self.at.x * s, self.at.y * s),
                    screen: (0, 0),
                }),
            ));
        }
    }
}
impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        a: &mut Self,
        _: &wl_pointer::WlPointer,
        e: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                serial,
            } => {
                a.serial = serial;
                a.pointer_focus = surface.data::<SurfaceId>().copied();
                a.at = Point::new(surface_x as f32, surface_y as f32);
                a.pointer_event(Event::Move);
            }
            wl_pointer::Event::Leave { .. } => {
                if let Some(id) = a.pointer_focus.take() {
                    a.events.push_back((id, Event::Leave));
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                a.at = Point::new(surface_x as f32, surface_y as f32);
                a.pointer_event(Event::Move);
            }
            wl_pointer::Event::Button {
                serial,
                button,
                state,
                ..
            } => {
                a.serial = serial;
                let b = match button {
                    272 => Button::Left,
                    273 => Button::Right,
                    274 => Button::Middle,
                    _ => return,
                };
                if b == Button::Left
                    && state == wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed)
                    && let Some(id) = a.pointer_focus
                    && let Some(n) = a.windows.get(&id)
                {
                    let at = Point::new(a.at.x * n.scale as f32, a.at.y * n.scale as f32);
                    if let Some((_, action)) =
                        n.chrome.iter().rev().find(|(rect, _)| rect.contains(at))
                        && let Shell::Top(_, top) = &n.shell
                    {
                        use super::app_window::Chrome;
                        match action {
                            Chrome::Caption => {
                                if let Some(seat) = &a.seat {
                                    top._move(seat, serial);
                                }
                            }
                            Chrome::Minimize => top.set_minimized(),
                            Chrome::Maximize => {
                                if n.maximized {
                                    top.unset_maximized();
                                } else {
                                    top.set_maximized();
                                }
                            }
                            Chrome::Close => a.events.push_back((id, Event::Close)),
                        }
                        return;
                    }
                }
                a.pointer_event(|p| {
                    if state == wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) {
                        Event::Press(p, b)
                    } else {
                        Event::Release(p, b)
                    }
                });
            }
            wl_pointer::Event::Axis {
                axis: wayland_client::WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => a.pointer_event(|p| Event::Wheel(p, (-value * 12.) as i16)),
            _ => {}
        }
    }
}
impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        a: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        e: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_keyboard::Event::Keymap {
                format: wayland_client::WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            } => {
                a.xkb = super::keyboard::Keyboard::new(fd, size as usize);
            }
            wl_keyboard::Event::Enter { surface, .. } => {
                a.key_focus = surface.data::<SurfaceId>().copied()
            }
            wl_keyboard::Event::Leave { .. } => {
                a.key_focus = None;
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(s) = &mut a.xkb {
                    s.modifiers(mods_depressed, mods_latched, mods_locked, group);
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                if let (Some(id), Some(s)) = (a.key_focus, &a.xkb) {
                    let sym = s.symbol(key);
                    if let Some(event) = super::text_field::key(id, sym, &s.text(key)) {
                        a.events.push_back((id, event));
                    }
                    let vk = match sym {
                        0xff51 => 0x25,
                        0xff52 => 0x26,
                        0xff53 => 0x27,
                        0xff54 => 0x28,
                        0xff1b => 0x1b,
                        0xff0d => 0x0d,
                        0xff09 => 9,
                        0xff08 => 8,
                        0xff50 => 0x24,
                        0xff57 => 0x23,
                        32..=126 => {
                            if (97..=122).contains(&sym) {
                                sym - 32
                            } else {
                                sym
                            }
                        }
                        _ => return,
                    };
                    a.events.push_back((id, Event::Key(vk as u16)));
                }
            }
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(State: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(State: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(State: ignore xdg_positioner::XdgPositioner);
wayland_client::delegate_noop!(State: ignore layer_shell::ZwlrLayerShellV1);
