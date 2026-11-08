use crate::{DisplayList, HitId, Point, Ui};
use anyhow::{Context, Result, bail};
/// What kind of window a surface is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Parentless popup fallback (xdg toplevel). Use `Surface::popup` with a parent
    /// for a real xdg popup. Activation always belongs to the compositor.
    Popup { #[doc = "Whether the popup requests activation."] activate: bool },
    /// Layer-shell top-layer surface; unavailable on GNOME.
    Bar,
    /// Layer-shell background surface; `parent` is retained for source compatibility.
    DesktopChild { #[doc = "Native parent handle retained for desktop surface compatibility."] parent: usize },
    /// A normal xdg toplevel with client-side chrome. Minimum size is logical pixels.
    App { #[doc = "Minimum logical window size."] min: (u32, u32) },
}

/// Identifies a surface in [`Event`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(isize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Button.
pub enum Button {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Middle.
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
    /// Move.
    Move(Pointer),
    /// Leave.
    Leave,
    /// Positive delta scrolls up; position uses the same coordinates as other pointer events.
    Wheel(Pointer, i16),
    /// Capture lost.
    CaptureLost,
    /// Press.
    Press(Pointer, Button),
    /// Release.
    Release(Pointer, Button),
    /// Double click.
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
    /// Dpi changed.
    DpiChanged(u32),
    /// Visibility changed.
    VisibilityChanged,
    /// Cancelled.
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
    /// A character typed (a key's text, a compose sequence, or an input method's commit), for
    /// `el`'s text fields.
    Char(char),
    /// The input method's composition changed (`ime::preedit` has it).
    Preedit,
}

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    os::fd::{AsFd, AsRawFd, FromRawFd},
    rc::Rc,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_region,
        wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
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
    a11y: super::a11y::A11y,
    surface: wl_surface::WlSurface,
    shell: Shell,
    configured: bool,
    buffers: Option<Buffers>,
    #[cfg(feature = "gpu")]
    gpu: Option<super::gpu::Window>,
    scale: u32,
    animation: Option<(Rc<Cell<bool>>, Instant)>,
    callback: bool,
    chrome: Vec<(crate::Rect, super::app_window::Chrome)>,
    maximized: bool,
    size: (u32, u32),
}
impl Drop for Native {
    fn drop(&mut self) {
        #[cfg(feature = "gpu")]
        drop(self.gpu.take());
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
pub(crate) struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm: Option<xdg_wm_base::XdgWmBase>,
    layer: Option<layer_shell::ZwlrLayerShellV1>,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    xkb: Option<super::keyboard::Keyboard>,
    windows: HashMap<SurfaceId, Native>,
    pub(super) events: VecDeque<(SurfaceId, Event)>,
    next: isize,
    pub(super) scale: u32,
    screen_size: (i32, i32),
    at: Point,
    pointer_focus: Option<SurfaceId>,
    key_focus: Option<SurfaceId>,
    pub(super) serial: u32,
    pub(super) clip: super::clipboard::Clip,
    pub(super) ime: super::ime::Ime,
    /// Whether windows get AT-SPI adapters, and the session-bus socket that watches it.
    a11y: super::a11y::Gate,
}
pub(super) struct Runtime {
    pub(super) conn: Connection,
    pub(super) queue: EventQueue<State>,
    pub(super) state: State,
}
/// Runs `f` on this thread's Wayland connection (opened on first use).
pub(super) fn with<T>(f: impl FnOnce(&mut Runtime) -> Result<T>) -> Result<T> {
    runtime(f)
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
                clip: Default::default(),
                ime: Default::default(),
                a11y: super::a11y::Gate::new(),
            };
            queue.roundtrip(&mut state)?;
            queue.roundtrip(&mut state)?;
            let q = queue.handle();
            super::clipboard::start(&mut state.clip, state.seat.as_ref(), &q);
            super::ime::start(&mut state.ime, state.seat.as_ref(), &q);
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
    #[cfg(feature = "gpu")]
    let glass = n.gpu.as_ref().is_some_and(|g| g.animating());
    #[cfg(not(feature = "gpu"))]
    let glass = false;
    if !n.callback
        && (glass
            || n.animation
                .as_ref()
                .is_some_and(|(a, end)| a.get() && Instant::now() < *end))
    {
        n.surface.frame(q, id);
        n.callback = true;
    }
}
/// Takes one pending native event without blocking; sleep with `host::wait` when empty.
pub fn next_event() -> Option<(SurfaceId, Event)> {
    runtime(|r| {
        r.queue.dispatch_pending(&mut r.state)?;
        if r.state.events.is_empty() {
            r.conn.flush()?;
            if let Some(guard) = r.queue.prepare_read() {
                let mut fds = [
                    libc::pollfd { fd: guard.connection_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 },
                    libc::pollfd { fd: r.state.a11y.fd(), events: libc::POLLIN, revents: 0 },
                ];
                // SAFETY: fds holds two initialized pollfds; timeout zero only checks readiness.
                if unsafe { libc::poll(fds.as_mut_ptr(), 2, 0) } > 0 && fds[0].revents & libc::POLLIN != 0 {
                    guard.read()?;
                } else {
                    drop(guard);
                }
                if fds[1].revents != 0 {
                    r.state.accessibility_status();
                }
            }
            r.queue.dispatch_pending(&mut r.state)?;
        }
        r.state.accessibility_events();
        Ok(r.state.events.pop_front())
    })
    .unwrap_or_else(|e| {
        log::error!("Wayland dispatch: {e}");
        None
    })
}
/// Wayland's borrowed connection descriptor, for integrating stdin or other descriptors into
/// an application's poll loop. Dispatch with `next_event` when it becomes readable.
pub fn connection_fd() -> Result<std::os::fd::RawFd> {
    runtime(|r| Ok(r.conn.as_fd().as_raw_fd()))
}
/// Sleeps on the Wayland display fd until an application event is queued, or the deadline
/// `host::wake_at` set passes (its timerfd). No idle timer.
pub(super) fn wait() {
    if let Err(e) = runtime(|r| {
        loop {
            r.queue.dispatch_pending(&mut r.state)?;
            r.state.accessibility_events();
            if !r.state.events.is_empty() {
                return Ok(());
            }
            r.conn.flush()?;
            let Some(guard) = r.queue.prepare_read() else { continue };
            let timer = super::wake::armed();
            let mut fds = vec![
                libc::pollfd { fd: guard.connection_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 },
                libc::pollfd { fd: timer.unwrap_or(-1), events: libc::POLLIN, revents: 0 },
                libc::pollfd { fd: r.state.a11y.fd(), events: libc::POLLIN, revents: 0 },
                libc::pollfd { fd: super::wake::waker_fd().unwrap_or(-1), events: libc::POLLIN, revents: 0 },
            ];
            fds.extend(r.state.windows.values().filter_map(|n| {
                let fd = n.a11y.fd();
                (fd >= 0).then_some(libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
            }));
            // SAFETY: fds contains initialized pollfds; no timeout or idle timer.
            if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) } < 0 {
                continue;
            }
            if fds[0].revents & libc::POLLIN != 0 {
                match guard.read() {
                    Ok(_) => {}
                    Err(wayland_client::backend::WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e.into()),
                }
            } else {
                drop(guard);
            }
            // Hang-ups count too: the gate then closes the socket, so poll can't spin on it.
            if fds[2].revents != 0 {
                r.state.accessibility_status();
            }
            if fds[1].revents & libc::POLLIN != 0 {
                super::wake::fired();
                r.queue.dispatch_pending(&mut r.state)?;
                return Ok(());
            }
            if fds[3].revents & libc::POLLIN != 0 {
                super::wake::woken();
                r.queue.dispatch_pending(&mut r.state)?;
                return Ok(());
            }
        }
    }) {
        log::error!("Wayland wait: {e}");
    }
    crate::waker::rearm();
}
pub(super) fn screen_size() -> (i32, i32) {
    runtime(|r| Ok(r.state.screen_size)).unwrap_or((1920, 1080))
}
pub(super) fn shift_down() -> bool {
    runtime(|r| Ok(r.state.xkb.as_ref().is_some_and(|k| k.shift_down()))).unwrap_or(false)
}
pub(super) fn modifiers() -> crate::el::Modifiers {
    runtime(|r| Ok(r.state.xkb.as_ref().map(|k| k.modifiers_held()).unwrap_or_default())).unwrap_or_default()
}
pub(crate) fn queue_for(id: SurfaceId, event: Event) {
    queue_event(id.0 as usize, event);
}
/// Queue event.
pub fn queue_event(id: usize, event: Event) {
    let _ = runtime(|r| {
        r.state.events.push_back((SurfaceId(id as isize), event));
        Ok(())
    });
}
/// A Wayland surface with a retained display list and a CPU bitmap or Vulkan texture.
pub struct Surface {
    id: SurfaceId,
    alpha: u8,
    shown: DisplayList,
    canvas: Option<super::render::Canvas>,
    origin: (i32, i32),
    #[cfg(feature = "gpu")]
    glass: RefCell<super::gpu::GlassState>,
    mode: crate::platform::Render,
    size: (u32, u32),
}
impl Surface {
    /// Creates a new instance.
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
        role: accesskit::Role,
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
                    crate::identity::current().prefix.into(),
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
                    t.set_app_id(crate::identity::current().prefix.into());
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
                    a11y: super::a11y::A11y::new(role, name, r.state.a11y.enabled()),
                    surface,
                    shell,
                    configured: false,
                    buffers: None,
                    #[cfg(feature = "gpu")]
                    gpu: None,
                    scale: r.state.scale,
                    animation: None,
                    callback: false,
                    chrome: Vec::new(),
                    maximized: false,
                    size: (0, 0),
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
                #[cfg(feature = "gpu")]
                glass: RefCell::default(),
                mode: crate::platform::render()
                    .resolve(crate::glass::wanted(), crate::glass::cpu_backdrop()),
                size: (0, 0),
            })
        })
    }
    /// New deferred accessibility.
    pub fn new_deferred_accessibility(k: Kind, n: &str, r: accesskit::Role) -> Result<Self> {
        Self::new(k, n, r)
    }
    /// Present.
    pub fn present(
        &mut self,
        ui: &mut Ui,
        origin: (i32, i32),
        size: (u32, u32),
        list: DisplayList,
    ) -> Result<bool> {
        runtime(|r| {
            r.state.windows.get_mut(&self.id).context("surface released")?.a11y.update(&list.hits, list.focus);
            Ok(())
        })?;
        let resized = self.size != size;
        #[cfg(feature = "gpu")]
        let glass_changed = self.mode == crate::platform::Render::Gpu
            && runtime(|r| {
                Ok(r.state.windows[&self.id]
                    .gpu
                    .as_ref()
                    .is_none_or(|g| g.glass_revision() != self.glass.borrow().revision))
            })?;
        #[cfg(not(feature = "gpu"))]
        let glass_changed = false;
        let damage = if resized || glass_changed {
            Some(crate::Rect::new(0., 0., size.0 as f32, size.1 as f32))
        } else {
            list.damage(&self.shown)
        };
        self.origin = origin;
        if let Some(area) = damage.and_then(|d| d.pixels(size.0, size.1)) {
            #[cfg(feature = "gpu")]
            if self.mode == crate::platform::Render::Gpu {
                runtime(|r| {
                    let n = r
                        .state
                        .windows
                        .get_mut(&self.id)
                        .context("surface released")?;
                    if n.gpu.is_none() {
                        let display = std::ptr::NonNull::new(r.conn.backend().display_ptr().cast())
                            .context("Wayland display pointer")?;
                        let surface = std::ptr::NonNull::new(n.surface.id().as_ptr().cast())
                            .context("Wayland surface pointer")?;
                        // SAFETY: Native owns Window, destroys it before wl_surface, and the
                        // thread's runtime connection outlives every Native.
                        let mut gpu = unsafe { super::gpu::Window::new(display, surface, size) }?;
                        gpu.opacity(self.alpha);
                        n.gpu = Some(gpu);
                    }
                    n.size = size;
                    n.surface.set_buffer_scale(n.scale as i32);
                    if let Shell::Top(xdg, _) = &n.shell {
                        xdg.set_window_geometry(
                            0,
                            0,
                            (size.0 / n.scale).max(1) as i32,
                            (size.1 / n.scale).max(1) as i32,
                        );
                    }
                    n.surface.damage_buffer(
                        area.x as i32,
                        area.y as i32,
                        area.width as i32,
                        area.height as i32,
                    );
                    frame(n, &r.queue.handle(), self.id);
                    n.gpu.as_mut().unwrap().draw(
                        ui,
                        &list,
                        size,
                        area,
                        &self.glass.borrow(),
                        n.scale as f32,
                    )?;
                    frame(n, &r.queue.handle(), self.id);
                    n.surface.commit();
                    r.conn.flush()?;
                    Ok(())
                })?;
                self.size = size;
                self.shown = list;
                return Ok(true);
            }
            if resized {
                self.canvas = Some(super::render::Canvas::new(ui, size.0, size.1)?);
            }
            let c = self.canvas.as_ref().unwrap();
            c.draw(&mut ui.text, &list, area)?;
            self.send(&c.image(), area)?;
            self.size = size;
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
            n.size = size;
            if let Shell::Top(xdg, _) = &n.shell {
                xdg.set_window_geometry(
                    0,
                    0,
                    (size.0 / n.scale).max(1) as i32,
                    (size.1 / n.scale).max(1) as i32,
                );
            }
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
    /// Reads the retained client frame (including in-process glass) for screenshots. GPU
    /// readback waits for submitted work; it is never part of normal presentation.
    pub fn image(&self) -> Result<crate::Image> {
        #[cfg(feature = "gpu")]
        if self.mode == crate::platform::Render::Gpu {
            return runtime(|r| {
                r.state.windows[&self.id]
                    .gpu
                    .as_ref()
                    .context("no GPU frame")?
                    .image()
            });
        }
        Ok(self.canvas.as_ref().context("no CPU frame")?.image())
    }
    /// Shown.
    pub fn shown(&self) -> &DisplayList {
        &self.shown
    }
    /// Stable identity used to match this item across frames.
    pub fn id(&self) -> SurfaceId {
        self.id
    }
    /// Origin.
    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }
    /// Scale.
    pub fn scale(&self) -> f32 {
        runtime(|r| Ok(r.state.windows[&self.id].scale as f32)).unwrap_or(1.0)
    }
    /// Sets the initial physical client size; Wayland controls the global position.
    pub fn place(&self, (_, _, width, height): (i32, i32, i32, i32)) {
        let _ = runtime(|r| {
            let n = r
                .state
                .windows
                .get_mut(&self.id)
                .context("surface released")?;
            n.size = (width.max(1) as u32, height.max(1) as u32);
            if let Shell::Top(xdg, _) = &n.shell {
                xdg.set_window_geometry(
                    0,
                    0,
                    (n.size.0 / n.scale).max(1) as i32,
                    (n.size.1 / n.scale).max(1) as i32,
                );
            }
            Ok(())
        });
    }
    /// Returns the current physical client dimensions.
    pub fn client_size(&self) -> (u32, u32) {
        runtime(|r| Ok(r.state.windows[&self.id].size)).unwrap_or((0, 0))
    }
    /// Maps the surface; activation remains compositor-controlled (no activation request).
    pub fn show_active(&self, _: bool) {
        self.show();
    }
    /// Marks an opaque app backing for the compositor; pixels come from the display list.
    pub fn set_background(&self, color: crate::Rgba) {
        let _ = runtime(|r| {
            let n = &r.state.windows[&self.id];
            if color.a >= 1.0 {
                let region = r
                    .state
                    .compositor
                    .as_ref()
                    .context("wl_compositor unavailable")?
                    .create_region(&r.queue.handle(), ());
                region.add(0, 0, i32::MAX, i32::MAX);
                n.surface.set_opaque_region(Some(&region));
                region.destroy();
            } else {
                n.surface.set_opaque_region(None);
            }
            Ok(())
        });
    }
    /// Animation frames.
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
    /// Sets alpha.
    pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
        if self.alpha != alpha {
            self.alpha = alpha;
            #[cfg(feature = "gpu")]
            if self.mode == crate::platform::Render::Gpu {
                return runtime(|r| {
                    if let Some(g) = r
                        .state
                        .windows
                        .get_mut(&self.id)
                        .and_then(|n| n.gpu.as_mut())
                    {
                        g.set_alpha(alpha)?;
                    }
                    r.conn.flush()?;
                    Ok(())
                });
            }
            if let Some(c) = &self.canvas {
                let image = c.image();
                self.send(&image, crate::PixelRect::full(image.width, image.height))?;
            }
        }
        Ok(())
    }
    /// Move to.
    pub fn move_to(&mut self, _: (i32, i32)) -> Result<()> {
        bail!("Wayland does not permit absolute window positioning")
    }
    /// Publishes semantic values only while an assistive client is active.
    pub fn set_accessibility_semantics(&self, semantics: Vec<(crate::HitId, crate::Semantics)>) {
        let _ = runtime(|r| { if let Some(n) = r.state.windows.get_mut(&self.id) { n.a11y.semantics(semantics); } Ok(()) });
    }
    /// Takes a rich action accompanying an accessibility Invoke event.
    pub fn take_accessibility_action(&self, hit: crate::HitId) -> Option<crate::AccessibilityAction> {
        runtime(|r| Ok(r.state.windows.get_mut(&self.id).and_then(|n| n.a11y.take(hit)))).ok().flatten()
    }
    /// Sets glass.
    pub fn set_glass(&self, glass: Option<crate::Glass>) {
        self.set_glass_group(glass.as_slice(), 16.0);
    }
    /// Sets glass style.
    pub fn set_glass_style(&self, style: crate::glass::GlassStyle) {
        #[cfg(feature = "gpu")]
        {
            let mut g = self.glass.borrow_mut();
            if g.style != style {
                g.style = style;
                g.revision += 1;
            }
        }
        #[cfg(not(feature = "gpu"))]
        let _ = style;
    }
    /// Supplies client-owned premultiplied BGRA pixels to refract. No desktop capture is possible
    /// on Wayland; without this bitmap GPU glass uses a generated test pattern.
    #[cfg(feature = "gpu")]
    pub fn set_glass_backdrop(&self, image: crate::Image) -> Result<()> {
        anyhow::ensure!(
            image.width > 0
                && image.height > 0
                && image.pixels.len() == image.width as usize * image.height as usize * 4,
            "invalid glass backdrop"
        );
        let mut g = self.glass.borrow_mut();
        g.backdrop = Some(image);
        g.backdrop_revision += 1;
        g.revision += 1;
        Ok(())
    }
    /// Chooses CPU or Vulkan independently of other surfaces on this thread.
    pub fn with_render(
        kind: Kind,
        name: &str,
        role: accesskit::Role,
        mode: crate::platform::Render,
    ) -> Result<Self> {
        let mut surface = Self::new(kind, name, role)?;
        surface.mode = mode.resolve(crate::glass::wanted(), crate::glass::cpu_backdrop());
        Ok(surface)
    }
    /// Render.
    pub fn render(&self) -> crate::platform::Render {
        self.mode
    }
    /// In-process glass is available only on a GPU surface.
    pub fn glass_available(&self) -> bool {
        self.mode == crate::platform::Render::Gpu
    }
    /// Sets glass group.
    pub fn set_glass_group(&self, glasses: &[crate::Glass], spacing: f32) {
        #[cfg(feature = "gpu")]
        {
            let mut g = self.glass.borrow_mut();
            if g.glasses != glasses || g.spacing != spacing {
                g.glasses = glasses.to_vec();
                g.spacing = spacing;
                g.revision += 1;
            }
        }
        #[cfg(not(feature = "gpu"))]
        let _ = (glasses, spacing);
    }
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
                #[cfg(feature = "gpu")]
                if self.mode == crate::platform::Render::Gpu {
                    let result = runtime(|r| {
                        if let Some(g) = r
                            .state
                            .windows
                            .get_mut(&self.id)
                            .and_then(|n| n.gpu.as_mut())
                        {
                            g.present()?;
                        }
                        r.conn.flush()?;
                        Ok(())
                    });
                    if let Err(e) = result {
                        log::error!("Wayland GPU show: {e}");
                    }
                }
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
    /// Hide.
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
    /// Release.
    pub fn release(&mut self) {
        let _ = runtime(|r| {
            r.state.windows.remove(&self.id);
            r.conn.flush()?;
            Ok(())
        });
    }
    /// Discard events.
    pub fn discard_events(&self) {
        let _ = runtime(|r| {
            r.state.events.retain(|(id, _)| *id != self.id);
            Ok(())
        });
    }
    /// Takes focus from the pending state.
    pub fn take_focus(&self) -> &'static str {
        "Wayland focus is compositor controlled"
    }
    /// The pointer's shape over the surface. Wayland surfaces keep the compositor's default
    /// until the cursor-shape-v1 protocol is wired up.
    pub fn set_cursor(&self, _: super::app_window::Cursor) {}
    /// Sets chrome.
    pub fn set_chrome(&self, parts: &[(crate::Rect, super::app_window::Chrome)]) {
        let _ = runtime(|r| {
            if let Some(n) = r.state.windows.get_mut(&self.id) {
                n.chrome = parts.to_vec();
            }
            Ok(())
        });
    }
    /// Minimize.
    pub fn minimize(&self) -> Result<()> {
        runtime(|r| {
            if let Shell::Top(_, t) = &r.state.windows[&self.id].shell {
                t.set_minimized();
            }
            r.conn.flush()?;
            Ok(())
        })
    }
    /// Toggle maximize.
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
    /// Start resize.
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
                "wl_data_device_manager" => a.clip.manager = Some(r.bind(name, version.min(3), q, ())),
                "zwp_text_input_manager_v3" => a.ime.manager = Some(r.bind(name, 1, q, ())),
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
                    if let Some(n) = a.windows.get_mut(id) {
                        n.size = (width as u32 * s, height as u32 * s);
                    }
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
                n.size = (n.size.0 / n.scale * scale, n.size.1 / n.scale * scale);
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
            #[cfg(feature = "gpu")]
            if n.configured
                && let Some(g) = n.gpu.as_mut()
                && g.animating()
            {
                if let Err(e) = g.present() {
                    log::error!("Wayland glass frame: {e}");
                }
                frame(n, q, *id);
                n.surface.commit();
            }
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
    /// The status socket is readable: bring every window's adapter up or down if the answer changed.
    fn accessibility_status(&mut self) {
        if let Some(enabled) = self.a11y.ready() {
            for native in self.windows.values_mut() {
                native.a11y.set_enabled(enabled);
            }
        }
    }
    fn accessibility_events(&mut self) {
        for (id, native) in &mut self.windows {
            self.events.extend(native.a11y.drain().into_iter().map(|event| (*id, event)));
        }
    }
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
                a.key_focus = surface.data::<SurfaceId>().copied();
                if let Some(n) = a.key_focus.and_then(|id| a.windows.get_mut(&id)) { n.a11y.focus(true); }
            }
            wl_keyboard::Event::Leave { .. } => {
                if let Some(n) = a.key_focus.and_then(|id| a.windows.get_mut(&id)) { n.a11y.focus(false); }
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
                serial,
                state: wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                // Copying puts our selection up with the latest input's serial.
                a.serial = serial;
                if let (Some(id), Some(s)) = (a.key_focus, &mut a.xkb) {
                    let sym = s.symbol(key);
                    if let Some(event) = super::text_field::key(id, sym, &s.text(key)) {
                        a.events.push_back((id, event));
                    }
                    let vk = match sym {
                        0xff51 => Some(0x25),
                        0xff52 => Some(0x26),
                        0xff53 => Some(0x27),
                        0xff54 => Some(0x28),
                        0xff1b => Some(0x1b),
                        0xff0d => Some(0x0d),
                        0xff09 => Some(9),
                        0xff08 => Some(8),
                        0xff50 => Some(0x24),
                        0xff57 => Some(0x23),
                        0xff55 => Some(0x21),
                        0xff56 => Some(0x22),
                        0xff63 => Some(0x2d),
                        0xffff => Some(0x2e),
                        0xff67 => Some(0x5d),
                        // Letters, digits and space are their own virtual keys; other printable
                        // symbols ('(' would be 0x28, Down's code) type characters only.
                        32 | 48..=57 | 65..=90 => Some(sym),
                        97..=122 => Some(sym - 32),
                        _ => None,
                    };
                    if let Some(vk) = vk {
                        a.events.push_back((id, Event::Key(vk as u16)));
                    }
                    // What the key types, through compose (dead keys, Compose sequences), as
                    // characters after the key, as Windows sends WM_CHAR after WM_KEYDOWN.
                    let m = s.modifiers_held();
                    if let super::keyboard::Typed::Text(text) = s.typed(key)
                        && !(m.ctrl || m.alt || m.meta)
                    {
                        a.events.extend(text.chars().filter(|c| !c.is_control()).map(|c| (id, Event::Char(c))));
                    }
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

wayland_client::delegate_noop!(State: ignore wl_region::WlRegion);

impl Surface {
    /// Native layers aren't available here yet (on Wayland they will be subsurfaces below the
    /// surface's buffer): this fails, and a [`crate::el::native`] element stays a transparent hole.
    pub fn native_layer(&self, name: &str) -> anyhow::Result<crate::NativeHandle> {
        anyhow::bail!("native layer {name}: not available on this platform yet")
    }

    /// Nothing to place (see [`native_layer`](Self::native_layer)).
    pub fn place_native_layer(&self, _: &str, _: Option<crate::Rect>) {}

    /// Nothing to release (see [`native_layer`](Self::native_layer)).
    pub fn release_native_layer(&self, _: &str) {}
}

impl Surface {
    /// The title area's height where the system draws window controls: only macOS does, so
    /// this does nothing here (apps draw their own controls with [`Chrome`](crate::Chrome)).
    pub fn set_caption(&self, _: crate::Caption) {}

    /// Where the system draws the window controls: nowhere here (apps draw their own).
    pub fn system_controls(&self) -> Option<crate::Rect> {
        None
    }
}
