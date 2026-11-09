//! A surface: one per-pixel-alpha layered window, updated with `UpdateLayeredWindowIndirect`
//! over the dirty rectangle of each new display list.
//!
//! Input never runs app code inside a window procedure: the procedure turns messages into
//! [`Event`]s on this thread's queue ([`next_event`]), and the app's loop handles them after
//! dispatching. So app state is never borrowed twice, even when Windows sends a message from
//! inside one of our own calls (showing, focusing, a COM call to Explorer).
//!
//! Hit-testing follows alpha: pixels at alpha 0 let clicks through, anything else is ours. Views
//! fill a surface's whole shape with at least [`Rgba::HIT`](crate::Rgba::HIT).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use accesskit::Role;
use anyhow::{Context, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SIZE,
    WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, ScreenToClient, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GWLP_USERDATA,
    GetForegroundWindow, GetWindowLongPtrW, GetWindowThreadProcessId, HICON, IDC_ARROW,
    IsWindowVisible, LoadCursorW, MA_ACTIVATE, MA_NOACTIVATE, RegisterClassExW, SW_HIDE,
    SW_SHOWNOACTIVATE, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, ULW_ALPHA,
    UPDATELAYEREDWINDOWINFO, UpdateLayeredWindowIndirect, WA_INACTIVE, WINDOW_EX_STYLE,
    WM_ACTIVATE, WM_CAPTURECHANGED, WM_GETOBJECT, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SETFOCUS, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::HSTRING;

use super::a11y::{A11y, WM_A11Y_ACTION};
use crate::geom::{PixelRect, Point};
use crate::render::{Canvas, Ui};
use crate::scene::{DisplayList, HitId};

const WM_MOUSELEAVE: u32 = 0x02A3;

/// What kind of window a surface is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A topmost popup: menus, flyouts. With `activate` it takes the keyboard focus when
    /// asked ([`Surface::take_focus`]); without, clicking it never activates it (submenus).
    Popup { #[doc = "Whether the popup requests activation."] activate: bool },
    /// A floating bar: topmost and never activated. Its app-bar reservation
    /// (`SHAppBarMessage`) belongs to the bar (N2).
    Bar,
    /// Desktop-layer surface. The name is retained for callers, but this is a top-level
    /// layered tool window anchored immediately above `parent` (Progman), never a WS_CHILD.
    /// Raised desktops do not display ULW children, and WGC cannot exclude child windows.
    DesktopChild { #[doc = "Native parent handle retained for desktop surface compatibility."] parent: HWND },
    /// A normal app window (Studio): resizable, activated, in the taskbar, drawn opaque.
    /// `min` is its smallest client size in design pixels. See [`crate::app_window`].
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
    /// Files dragged from another app are over the surface ([`take_dropped_files`] after
    /// [`Event::FilesDropped`]).
    FilesHover(Pointer),
    /// The dragged files left the surface, or the drag was cancelled.
    FilesLeave,
    /// Files were dropped here; [`take_dropped_files`] has their paths.
    FilesDropped(Pointer),
    /// A key went down (a virtual-key code).
    Key(u16),
    /// A key went up (a virtual-key code): held-key input such as fly navigation pairs it
    /// with [`Event::Key`]. Repeats send `Key` again with no `KeyUp` between.
    KeyUp(u16),
    /// The surface became the active window. Desktop surfaces: Explorer may finish reordering
    /// its band afterwards. App windows: anything kept beside it in the z-order (a glass
    /// plate below it) needs restacking now, since activation raised it.
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
}

/// Stops a temporary frame producer when its surface's animation ends or closes.
pub struct AnimationFrames(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for AnimationFrames {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
const WM_FRAME: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 30;

thread_local! {
    static EVENTS: RefCell<VecDeque<(SurfaceId, Event)>> = const { RefCell::new(VecDeque::new()) };
}

/// The oldest input not handled yet, for any surface of this thread. Drain it after
/// dispatching window messages.
pub fn next_event() -> Option<(SurfaceId, Event)> {
    EVENTS.with(|queue| queue.borrow_mut().pop_front())
}

/// Queues semantic input from an app-owned IPC window on this UI thread. It bypasses
/// native keyboard/text dispatch; the caller must own `hwnd` on the calling thread.
pub fn queue_event(hwnd: HWND, event: Event) {
    push(SurfaceId(hwnd.0 as isize), event);
}

pub(crate) fn push(id: SurfaceId, event: Event) {
    EVENTS.with(|queue| queue.borrow_mut().push_back((id, event)));
}

/// What the window procedure shares with the [`Surface`].
pub(crate) struct Shared {
    id: SurfaceId,
    /// App windows only.
    pub(crate) app: Option<crate::app_window::AppShared>,
    activate: bool,
    no_activate: Cell<bool>,
    normally_no_activate: bool,
    desktop: Option<HWND>,
    restacking: Cell<bool>,
    /// Where a left click activates a never-activated window ([`Surface::set_click_activates`]).
    activates: RefCell<Vec<crate::geom::Rect>>,
    field: RefCell<Option<crate::TextField>>,
    /// The window's top-left on screen, for pointer events.
    origin: Cell<(i32, i32)>,
    a11y: RefCell<Option<A11y>>,
    semantics: RefCell<Vec<(HitId, crate::Semantics)>>,
    /// A GPU surface's drawn bounds (surface px), for `WM_NCHITTEST`: without a layered bitmap
    /// Windows can't hit-test by alpha. `None` on CPU surfaces.
    #[cfg(feature = "gpu")]
    solid: RefCell<Option<Vec<crate::geom::Rect>>>,
}

/// A GPU surface's composition and its in-process glass (`gpu`, `gpu_glass`).
#[cfg(feature = "gpu")]
struct Gpu {
    /// Made on the first present (it needs the `Ui`'s text parameters).
    layer: Option<super::gpu::Layer>,
    glass: Option<super::gpu_glass::Shared>,
    /// The glass wanted, and the distance within which it fuses (px).
    glasses: Vec<crate::glass::Glass>,
    spacing: f32,
}

/// The bitmap `UpdateLayeredWindowIndirect` reads: a DIB section selected into a memory DC.
struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u8,
    width: u32,
}

impl Dib {
    fn new(width: u32, height: u32) -> Result<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: GDI objects created here and released in `Drop`; `bits` is the section's memory.
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .inspect_err(|_| {
                    let _ = DeleteDC(dc);
                })
                .context("DIB section")?;
            let previous = SelectObject(dc, bitmap.into());
            Ok(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
            })
        }
    }

    /// Copies one row of `area` (premultiplied BGRA) into place.
    fn put_row(&mut self, area: PixelRect, row: u32, bytes: &[u8]) {
        let stride = self.width as usize * 4;
        let at = (area.y + row) as usize * stride + area.x as usize * 4;
        // SAFETY: `area` lies inside the bitmap, so `at..at + bytes.len()` is inside its memory.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.bits.add(at), bytes.len()) };
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        // SAFETY: our own GDI objects, released once.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

/// One layered window and what it shows.
pub struct Surface {
    hwnd: HWND,
    pub(crate) shared: Rc<Shared>,
    pixels: Option<(Canvas, Dib)>,
    shown: DisplayList,
    origin: (i32, i32),
    size: (u32, u32),
    alpha: u8,
    /// The engine glass wanted under the body, and whether it is open (only while shown).
    glass: Cell<Option<crate::glass::Glass>>,
    glass_open: Cell<bool>,
    glass_style: Cell<crate::glass::GlassStyle>,
    /// `Some` for a GPU surface.
    #[cfg(feature = "gpu")]
    gpu: Option<RefCell<Gpu>>,
}

impl Surface {
    /// Posts display-rate frames from a high-resolution waitable timer for this animation only.
    pub fn animation_frames(&self, duration: std::time::Duration) -> AnimationFrames {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let active = Arc::new(AtomicBool::new(true));
        let running = active.clone();
        let target = self.hwnd.0 as isize;
        let _ = std::thread::Builder::new()
            .name("ui-motion".into())
            .spawn(move || {
                let Ok(mut ticker) = crate::Ticker::new() else {
                    return;
                };
                let started = std::time::Instant::now();
                while running.load(Ordering::Relaxed) && started.elapsed() <= duration {
                    ticker.schedule(true);
                    // SAFETY: this worker owns its timer; the bounded wait ends with the next frame.
                    unsafe {
                        windows::Win32::System::Threading::WaitForSingleObject(
                            ticker.handle(),
                            100,
                        );
                    }
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    // SAFETY: posting is safe across threads and a destroyed window simply refuses.
                    let _ = unsafe {
                        windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                            Some(HWND(target as _)),
                            WM_FRAME,
                            WPARAM(0),
                            LPARAM(0),
                        )
                    };
                }
            });
        AnimationFrames(active)
    }
    /// A hidden, empty window. `name` and `role` describe it to assistive technology. It draws
    /// in this thread's default mode ([`crate::platform::set_render`], `Cpu` unless set).
    pub fn new(kind: Kind, name: &str, role: Role) -> Result<Self> {
        Self::create(kind, name, role, false, default_render())
    }

    /// [`Surface::new`] with its own render mode, whatever the thread's default.
    pub fn with_render(kind: Kind, name: &str, role: Role, render: crate::platform::Render) -> Result<Self> {
        Self::create(kind, name, role, false, render)
    }

    /// A startup surface whose UI Automation adapter is created on its first WM_GETOBJECT.
    /// Its latest hit snapshot is retained from the first present, including before activation.
    pub fn new_deferred_accessibility(kind: Kind, name: &str, role: Role) -> Result<Self> {
        Self::create(kind, name, role, true, default_render())
    }

    #[cfg_attr(not(feature = "gpu"), allow(unused_variables))]
    fn create(kind: Kind, name: &str, role: Role, deferred: bool, render: crate::platform::Render) -> Result<Self> {
        // GPU mode needs a hardware device, made (once per thread) by the first GPU surface; a
        // CPU surface never touches it. Without one the surface draws on the CPU.
        #[cfg(feature = "gpu")]
        let gpu = render
            .resolve(crate::glass::wanted(), crate::glass::cpu_backdrop())
            == crate::platform::Render::Gpu
            && super::gpu::device().is_some();
        #[cfg(not(feature = "gpu"))]
        let gpu = false;
        // A GPU window shows DirectComposition content only: no redirection bitmap, no layering.
        let layered = if gpu {
            windows::Win32::UI::WindowsAndMessaging::WS_EX_NOREDIRECTIONBITMAP
        } else {
            WS_EX_LAYERED
        };
        if let Kind::App { min } = kind {
            let ex = if gpu { layered } else { WINDOW_EX_STYLE::default() };
            let hwnd = crate::app_window::create(name, Some(wndproc), ex)?;
            // Files dropped from Explorer arrive as WM_DROPFILES. WS_EX_ACCEPTFILES is all
            // DragAcceptFiles sets; setting it here keeps shell32.dll out until a drop lands.
            // SAFETY: the window this call just made.
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::WS_EX_ACCEPTFILES;
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_ACCEPTFILES.0 as isize);
            }
            let app = crate::app_window::AppShared::new(min);
            return Ok(Self::adopt(hwnd, kind, name, role, deferred, Some(app), gpu));
        }
        register_class()?;
        let (style, ex, parent) = match kind {
            Kind::Popup { activate } => {
                let ex = layered | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
                (
                    WS_POPUP,
                    if activate { ex } else { ex | WS_EX_NOACTIVATE },
                    None,
                )
            }
            // No WS_EX_NOACTIVATE: `WM_MOUSEACTIVATE` decides per click (`set_click_activates`).
            Kind::Bar => (
                WS_POPUP,
                layered | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                None,
            ),
            Kind::DesktopChild { .. } => (
                WS_POPUP,
                // Receive normal click activation for keyboard editing. WM_WINDOWPOSCHANGING
                // below prevents activation from raising the surface out of its desktop band.
                layered | WS_EX_TOOLWINDOW,
                None,
            ),
            Kind::App { .. } => unreachable!("created above"),
        };
        // SAFETY: class registered above; position and size come with the first update.
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                crate::identity::surface_class(),
                &HSTRING::from(name),
                style,
                0,
                0,
                1,
                1,
                parent,
                None,
                None,
                None,
            )
        }
        .context("creating a surface window")?;
        Ok(Self::adopt(hwnd, kind, name, role, deferred, None, gpu))
    }

    /// Ties a new window to its shared state (the window procedure's view of the surface).
    #[cfg_attr(not(feature = "gpu"), allow(unused_variables))]
    fn adopt(
        hwnd: HWND,
        kind: Kind,
        name: &str,
        role: Role,
        deferred: bool,
        app: Option<crate::app_window::AppShared>,
        gpu: bool,
    ) -> Self {
        let activate = match kind {
            Kind::Popup { activate } => activate,
            Kind::App { .. } | Kind::DesktopChild { .. } => true,
            Kind::Bar => false,
        };
        let id = SurfaceId(hwnd.0 as isize);
        let is_app = app.is_some();
        let shared = Rc::new(Shared {
            id,
            app,
            activate,
            no_activate: Cell::new(false),
            normally_no_activate: matches!(kind, Kind::Popup { activate: false }),
            desktop: match kind {
                Kind::DesktopChild { parent } => Some(parent),
                _ => None,
            },
            restacking: Cell::new(false),
            activates: RefCell::new(Vec::new()),
            field: RefCell::new(None),
            origin: Cell::new((0, 0)),
            semantics: RefCell::default(),
            a11y: RefCell::new(Some(if deferred {
                A11y::deferred(hwnd, role, name.into())
            } else {
                A11y::new(hwnd, role, name.into())
            })),
            #[cfg(feature = "gpu")]
            solid: RefCell::new((gpu && !is_app).then(Vec::new)),
        });
        // The window holds a strong reference, released in WM_NCDESTROY.
        let raw = Rc::into_raw(shared.clone());
        // SAFETY: our own window; the pointer stays valid until WM_NCDESTROY takes it back.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw as isize);
            if shared.desktop.is_some() {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetPropW(
                    hwnd,
                    crate::identity::desktop_prop(),
                    Some(windows::Win32::Foundation::HANDLE(std::ptr::dangling_mut())),
                );
            }
        };
        if is_app {
            crate::app_window::created(hwnd);
        }
        Self {
            hwnd,
            shared,
            pixels: None,
            shown: DisplayList::default(),
            origin: (0, 0),
            size: (0, 0),
            alpha: 255,
            glass: Cell::new(None),
            glass_open: Cell::new(false),
            glass_style: Cell::new(crate::glass::GlassStyle::default()),
            #[cfg(feature = "gpu")]
            gpu: gpu.then(|| {
                RefCell::new(Gpu {
                    layer: None,
                    glass: None,
                    glasses: Vec::new(),
                    spacing: 0.0,
                })
            }),
        }
    }

    /// How this surface draws: `Cpu` or `Gpu` (`Auto` resolved when it was made).
    pub fn render(&self) -> crate::platform::Render {
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            return crate::platform::Render::Gpu;
        }
        crate::platform::Render::Cpu
    }

    /// Whether glass can show under this surface now: in-process glass on a GPU surface, else
    /// the thread's backdrop ([`crate::glass::available`]). The system backdrop never serves a
    /// CPU app window: it draws opaque, so frost under it could not show.
    pub fn glass_available(&self) -> bool {
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            return super::gpu_glass::device().is_some();
        }
        self.backdrop().is_some_and(|b| b.available())
    }

    /// Where this CPU surface's glass goes: the thread's backdrop, but never the system one for
    /// an app window.
    fn backdrop(&self) -> Option<Rc<dyn crate::glass::Backdrop>> {
        if self.shared.app.is_some() {
            crate::glass::installed()
        } else {
            crate::glass::backdrop()
        }
    }

    /// Stable identity used to match this item across frames.
    pub fn id(&self) -> SurfaceId {
        self.shared.id
    }

    /// Hwnd.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Attach a windowless field to receive raw keyboard, pointer and IME messages.
    /// Drop/blur it before hiding or destroying its surface.
    pub fn set_text_field(&self, field: Option<crate::TextField>) {
        if let Some(old) = self.shared.field.replace(field) {
            old.blur();
        }
    }

    /// Puts engine glass under `glass.body` (or takes it away, `None`). While the window shows,
    /// the glass follows it: opened on [`show`](Self::show), placed before every move, reshaped
    /// here, closed on [`hide`](Self::hide) and drop (crate::glass).
    ///
    /// On a GPU surface the glass is drawn in this process, under the surface's content, and
    /// follows the window by itself.
    pub fn set_glass(&self, glass: Option<crate::glass::Glass>) {
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &self.gpu {
            self.glass.set(glass);
            gpu.borrow_mut().glasses = glass.into_iter().collect();
            self.sync_glass();
            return;
        }
        let old = self.glass.replace(glass);
        if !self.glass_open.get() {
            if glass.is_some() && self.is_visible() {
                self.open_glass();
            }
            return;
        }
        let (Some(new), Some(backdrop)) = (glass, self.backdrop()) else {
            self.close_glass();
            return;
        };
        if old.is_none_or(|old| (old.body, old.radius) != (new.body, new.radius)) {
            backdrop.place(self.hwnd, new.shape(self.origin), new.radius);
        }
        if old.is_none_or(|old| old.material != new.material) {
            backdrop.material(self.hwnd, &new.material);
        }
    }

    /// Sets cached optics before showing a surface; changing them updates an open backdrop.
    pub fn set_glass_style(&self, style: crate::glass::GlassStyle) {
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            if self.glass_style.replace(style) != style {
                self.sync_glass();
            }
            return;
        }
        if self.glass_style.replace(style) != style
            && self.glass_open.get()
            && let Some(backdrop) = self.backdrop()
        {
            backdrop.style(self.hwnd, &style);
        }
    }

    /// Whether engine glass is open under the surface.
    pub fn has_glass(&self) -> bool {
        self.glass_open.get()
    }

    /// Several glass bodies under a GPU surface (surface px), in one liquid container: shapes
    /// within `spacing` px of each other fuse, and a body that moves morphs (springs) to its new
    /// frame, merging and splitting on the way. A CPU surface shows the first one, through its
    /// backdrop, as [`set_glass`](Self::set_glass) does.
    pub fn set_glass_group(&self, glasses: &[crate::glass::Glass], spacing: f32) {
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &self.gpu {
            self.glass.set(glasses.first().copied());
            let mut gpu = gpu.borrow_mut();
            gpu.glasses.clear();
            gpu.glasses.extend_from_slice(glasses);
            gpu.spacing = spacing;
            drop(gpu);
            self.sync_glass();
            return;
        }
        let _ = spacing;
        self.set_glass(glasses.first().copied());
    }

    /// Makes a GPU surface's in-process glass match what is wanted: shown while the window is
    /// visible and has glass, gone otherwise. Draws a frame when anything changed.
    #[cfg(feature = "gpu")]
    fn sync_glass(&self) {
        let Some(gpu) = &self.gpu else { return };
        let mut gpu = gpu.borrow_mut();
        let Gpu { layer, glass, glasses, spacing } = &mut *gpu;
        let wanted = !glasses.is_empty() && self.is_visible();
        match (wanted, layer.as_mut(), glass.as_ref()) {
            (true, Some(layer), None) => {
                let Some(device) = super::gpu_glass::device() else { return };
                let scale = scale_of(self.hwnd);
                // Under the content visual: the panel's labels draw over its glass.
                // SAFETY: a new visual of this thread's composition device, in our own tree.
                let visual = unsafe {
                    layer.device.dcomp.CreateVisual().and_then(|v| layer.root.AddVisual(&v, false, &layer.content).map(|()| v))
                };
                match visual.map_err(anyhow::Error::from).and_then(|v| super::gpu_glass::GlassLayer::new(device, v, self.size, scale)) {
                    Ok(mut new) => {
                        new.place(self.origin);
                        new.set(glasses, &self.glass_style.get(), *spacing);
                        let shared = Rc::new(RefCell::new(new));
                        super::gpu_glass::register(self.hwnd, &shared);
                        layer.touch();
                        if let Err(e) = layer.commit() {
                            log::error!("glass: {e:#}");
                        }
                        super::gpu_glass::draw(self.hwnd, &shared);
                        *glass = Some(shared);
                    }
                    Err(e) => log::error!("in-process glass: {e:#}"),
                }
            }
            (true, Some(_), Some(shared)) => {
                if let Ok(mut g) = shared.try_borrow_mut() {
                    g.set(glasses, &self.glass_style.get(), *spacing);
                }
                super::gpu_glass::draw(self.hwnd, shared);
            }
            (false, layer, Some(_)) => {
                super::gpu_glass::unregister(self.hwnd);
                *glass = None;
                if let Some(layer) = layer {
                    layer.touch();
                    let _ = layer.commit();
                }
            }
            _ => {}
        }
        self.glass_open.set(glass.is_some());
    }

    fn open_glass(&self) {
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            return self.sync_glass();
        }
        if let (Some(glass), Some(backdrop)) = (self.glass.get(), self.backdrop()) {
            let shape = glass.shape(self.origin);
            let opened = backdrop.open_styled(
                self.hwnd,
                shape,
                glass.radius,
                &glass.material,
                &self.glass_style.get(),
            );
            self.glass_open.set(opened);
            if opened {
                backdrop.opacity(self.hwnd, self.alpha);
                if self.is_visible() { backdrop.visibility(self.hwnd, true); }
            }
        }
    }

    fn close_glass(&self) {
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            return self.sync_glass();
        }
        if self.glass_open.replace(false)
            && let Some(backdrop) = self.backdrop()
        {
            backdrop.close(self.hwnd);
        }
    }

    /// The window's top-left on screen.
    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }

    /// What is on screen.
    pub fn shown(&self) -> &DisplayList {
        &self.shown
    }

    /// Shows `list` in a `size` window at `origin` (screen pixels). Draws only what differs
    /// from the list on screen; returns whether anything was drawn.
    pub fn present(
        &mut self,
        ui: &mut Ui,
        origin: (i32, i32),
        size: (u32, u32),
        list: DisplayList,
    ) -> Result<bool> {
        self.present_damage(ui, origin, size, list, false)
    }

    /// Paint disjoint changed regions (widget meters) with one layered-window present.
    pub fn present_sparse(
        &mut self,
        ui: &mut Ui,
        origin: (i32, i32),
        size: (u32, u32),
        list: DisplayList,
    ) -> Result<bool> {
        self.present_damage(ui, origin, size, list, true)
    }

    fn present_damage(
        &mut self,
        ui: &mut Ui,
        origin: (i32, i32),
        size: (u32, u32),
        mut list: DisplayList,
        sparse: bool,
    ) -> Result<bool> {
        // A clipped or removed control must never leave an invalid UI Automation focus node.
        list.focus = list
            .focus
            .filter(|id| list.hits.iter().any(|hit| hit.id == *id));
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            return self.present_gpu(ui, origin, size, list, sparse);
        }
        if self.shared.app.is_some() {
            return self.present_app(ui, size, list);
        }
        let resized = self.pixels.is_none() || size != self.size;
        if resized {
            self.pixels = None; // the old bitmaps go before the new ones come
            self.pixels = Some((Canvas::new(ui, size.0, size.1)?, Dib::new(size.0, size.1)?));
            self.size = size;
        }
        let regions = crate::damage::regions(&list, &self.shown, size, resized, sparse);
        let dirty = regions
            .iter()
            .map(|r| r.to_rect())
            .reduce(|a, b| a.union(b))
            .and_then(|r| r.pixels(size.0, size.1));
        if let (Some(_), Some((canvas, dib))) = (dirty, &mut self.pixels) {
            canvas.draw_regions(&mut ui.text, &list, &regions)?;
            canvas.read_regions(&regions, |area, row, bytes| dib.put_row(area, row, bytes))?;
        }
        if dirty.is_some() || origin != self.origin {
            let source = dirty.map(|area| (self.dib_dc(), area, resized));
            crate::platform::Window::present_bitmap(self, origin, source.as_ref().map(|(dc, area, resized)| (dc, *area, *resized)))?;
        }
        self.set_hits(&list);
        self.shown = list;
        Ok(dirty.is_some())
    }

    /// `present` for a GPU surface: draw the damage into the composition surface (one
    /// `BeginDraw` per region), move or resize the window, commit once. An unchanged frame at
    /// the same place draws and commits nothing.
    #[cfg(feature = "gpu")]
    fn present_gpu(
        &mut self,
        ui: &mut Ui,
        origin: (i32, i32),
        size: (u32, u32),
        list: DisplayList,
        sparse: bool,
    ) -> Result<bool> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        };
        let is_app = self.shared.app.is_some();
        let gpu = self.gpu.as_ref().expect("a GPU surface");
        let mut state = gpu.borrow_mut();
        if state.layer.is_none() {
            let device = super::gpu::device().context("GPU device")?;
            state.layer = Some(super::gpu::Layer::new(self.hwnd, device, ui.params())?);
        }
        let Gpu { layer, glass, .. } = &mut *state;
        let layer = layer.as_mut().expect("made above");
        let resized = size != self.size || !layer.has_content();
        if resized {
            layer.resize(size)?;
            if let Some(app) = &self.shared.app {
                app.size.set(size);
            }
        }
        let regions = crate::damage::regions(&list, &self.shown, size, resized, sparse);
        layer.draw(&mut ui.text, &list, &regions)?;
        let moved = !is_app && origin != self.origin;
        if !is_app && (moved || resized) {
            let flags = SWP_NOZORDER | SWP_NOACTIVATE | if resized { Default::default() } else { SWP_NOSIZE };
            // SAFETY: our own window.
            unsafe { SetWindowPos(self.hwnd, None, origin.0, origin.1, size.0 as i32, size.1 as i32, flags)? };
            self.origin = origin;
            self.shared.origin.set(origin);
        }
        self.size = size;
        if let Some(glass) = glass
            && (moved || resized)
        {
            if let Ok(mut g) = glass.try_borrow_mut() {
                g.resize(size);
                g.place(origin);
            }
            super::gpu_glass::draw(self.hwnd, glass);
        }
        layer.commit()?;
        drop(state);
        if let Some(solid) = self.shared.solid.borrow_mut().as_mut() {
            solid.clear();
            solid.extend(list.ops.iter().filter(|op| !matches!(op, crate::Op::Clip { .. } | crate::Op::Unclip)).map(crate::Op::bounds));
        }
        // The first present of a surface already shown with glass wanted.
        if !self.glass_open.get() && self.glass.get().is_some() {
            self.sync_glass();
        }
        self.set_hits(&list);
        self.shown = list;
        Ok(!regions.is_empty())
    }

    /// Moves the window without redrawing (slides).
    pub fn move_to(&mut self, origin: (i32, i32)) -> Result<()> {
        #[cfg(feature = "gpu")]
        if self.gpu.is_some() {
            if origin != self.origin {
                self.move_gpu(origin, self.size)?;
            }
            return Ok(());
        }
        if origin != self.origin {
            self.update(origin, None)?;
        }
        Ok(())
    }

    /// A GPU surface's move: the window (its glass moves with it), and the glass's backdrop.
    #[cfg(feature = "gpu")]
    fn move_gpu(&mut self, origin: (i32, i32), size: (u32, u32)) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        };
        // SAFETY: our own window.
        unsafe {
            SetWindowPos(self.hwnd, None, origin.0, origin.1, size.0 as i32, size.1 as i32, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE)?
        };
        self.origin = origin;
        self.shared.origin.set(origin);
        let glass = self.gpu.as_ref().and_then(|g| g.borrow().glass.clone());
        if let Some(glass) = glass {
            if let Ok(mut g) = glass.try_borrow_mut() {
                g.place(origin);
            }
            super::gpu_glass::draw(self.hwnd, &glass);
        }
        Ok(())
    }

    /// Fades the whole window without redrawing.
    pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &self.gpu {
            if alpha != self.alpha {
                self.alpha = alpha;
                if let Some(layer) = gpu.borrow_mut().layer.as_mut() {
                    layer.set_opacity(f32::from(alpha) / 255.0)?;
                    layer.commit()?;
                }
            }
            return Ok(());
        }
        if alpha != self.alpha {
            if self.glass_open.get() && let Some(backdrop) = self.backdrop() {
                backdrop.opacity(self.hwnd, alpha);
            }
            self.alpha = alpha;
            self.update(self.origin, None)?;
        }
        Ok(())
    }

    /// Shows the window without activating it.
    pub fn show(&self) {
        let native = self.backdrop().is_some_and(|b| b.synchronous());
        if native && !self.glass_open.get() { self.open_glass(); }
        // SAFETY: read-only window queries and a show of our own window, without activation.
        unsafe {
            if (!IsWindowVisible(self.hwnd).as_bool()
                || windows::Win32::UI::WindowsAndMessaging::IsIconic(self.hwnd).as_bool())
                && !self.backdrop().is_some_and(|b| b.visibility(self.hwnd, true))
            {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }
        if self.shared.desktop.is_some() {
            let _ = self.keep_above_desktop();
        }
        // After the content is on screen: the request may wait on the engine's loop.
        if !self.glass_open.get() {
            self.open_glass();
        }
    }

    /// Keep a desktop surface and its directly-underlying engine glass in the bottom band.
    /// Returns true only when a z-order repair was necessary. Coordinates never change here.
    pub fn keep_above_desktop(&self) -> Result<bool> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GW_HWNDNEXT, GW_HWNDPREV, GetWindow, HWND_TOP, IsWindow, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SetWindowPos,
        };
        let Some(anchor) = self.shared.desktop else {
            return Ok(false);
        };
        // SAFETY: handle queries only. A destroyed Explorer anchor is resolved by the host's
        // DesktopEvents before it creates replacement surfaces.
        unsafe {
            if !IsWindow(Some(anchor)).as_bool() {
                return Ok(false);
            }
            let next = GetWindow(self.hwnd, GW_HWNDNEXT).ok();
            let mut below = next;
            while let Some(window) = below {
                if window == anchor {
                    return Ok(false);
                }
                if !desktop_member(window) {
                    break;
                }
                below = GetWindow(window, GW_HWNDNEXT).ok();
            }
            self.shared.restacking.set(true);
            let result = (|| -> windows::core::Result<()> {
                let glass = next.filter(|&window| is_desktop_glass(window));
                let last = GetWindow(anchor, GW_HWNDPREV)
                    .ok()
                    .filter(|&w| w != self.hwnd)
                    .unwrap_or(HWND_TOP);
                let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
                if let Some(glass) = glass {
                    SetWindowPos(glass, Some(last), 0, 0, 0, 0, flags)?;
                    let before = GetWindow(glass, GW_HWNDPREV)
                        .ok()
                        .filter(|&w| w != self.hwnd)
                        .unwrap_or(HWND_TOP);
                    SetWindowPos(self.hwnd, Some(before), 0, 0, 0, 0, flags)?;
                } else {
                    SetWindowPos(self.hwnd, Some(last), 0, 0, 0, 0, flags)?;
                }
                Ok(())
            })();
            self.shared.restacking.set(false);
            result.context("restacking a desktop surface")?;
        }
        Ok(true)
    }

    /// Read-only z-order probe for live harnesses. Desktop windows must be in the contiguous
    /// widget/glass band over their Explorer anchor, below ordinary applications.
    pub fn desktop_order_valid(&self) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{GW_HWNDNEXT, GetWindow};
        let Some(anchor) = self.shared.desktop else {
            return true;
        };
        // SAFETY: read-only z-order walk. GetWindow ends the walk when a handle disappears.
        unsafe {
            let mut next = GetWindow(self.hwnd, GW_HWNDNEXT).ok();
            while let Some(window) = next {
                if window == anchor {
                    return true;
                }
                if !desktop_member(window) {
                    return false;
                }
                next = GetWindow(window, GW_HWNDNEXT).ok();
            }
        }
        false
    }

    /// Put edit chrome above the other desktop surfaces, while leaving every app above it.
    pub fn raise_in_desktop_band(&self) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GW_HWNDPREV, GetWindow, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        };
        let Some(anchor) = self.shared.desktop else {
            return Ok(());
        };
        // SAFETY: z-order queries and moves on our own window. The guard permits this explicit
        // repair through WM_WINDOWPOSCHANGING, which otherwise refuses automatic raises.
        unsafe {
            let mut top = anchor;
            while let Ok(previous) = GetWindow(top, GW_HWNDPREV) {
                if !desktop_member(previous) {
                    break;
                }
                top = previous;
            }
            if top == self.hwnd {
                return Ok(());
            }
            let before = GetWindow(top, GW_HWNDPREV).unwrap_or(HWND_TOP);
            self.shared.restacking.set(true);
            let result = SetWindowPos(
                self.hwnd,
                Some(before),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            self.shared.restacking.set(false);
            result?;
        }
        Ok(())
    }

    /// Hide.
    pub fn hide(&self) {
        if !self.backdrop().is_some_and(|b| b.visibility(self.hwnd, false)) {
            // SAFETY: our own window.
            let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }
        self.close_glass();
    }

    /// Whether visible.
    pub fn is_visible(&self) -> bool {
        // SAFETY: a query on our own window.
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }

    /// Lets clicks through the whole window (`true`), or only through alpha 0 (`false`).
    pub fn set_click_through(&self, through: bool) {
        // SAFETY: our own window's extended style.
        unsafe {
            let ex = WINDOW_EX_STYLE(GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32);
            let ex = if through {
                ex | WS_EX_TRANSPARENT
            } else {
                ex & !WS_EX_TRANSPARENT
            };
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex.0 as isize);
        }
    }

    /// Frees the bitmaps (a hidden surface keeps none); the next [`present`](Self::present)
    /// draws everything again.
    pub fn release(&mut self) {
        if let Some(app) = &self.shared.app {
            app.release_canvas();
        }
        // The composition and its glass go with the pixels; the next present makes them again.
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &self.gpu {
            super::gpu_glass::unregister(self.hwnd);
            let mut gpu = gpu.borrow_mut();
            gpu.glass = None;
            gpu.layer = None;
            self.glass_open.set(false);
        }
        self.pixels = None;
        self.shown = DisplayList::default();
        self.size = (0, 0);
    }

    /// Drops input queued for this surface and not handled yet (it was hidden or reused).
    pub fn discard_events(&self) {
        let id = self.id();
        EVENTS.with(|queue| queue.borrow_mut().retain(|(of, _)| *of != id));
    }

    /// Where a left click activates this window (surface pixels), for a window that is
    /// otherwise never activated (a bar). Windows never refuses an activation by click, so a
    /// bar item that opens a flyout or brings an app forward claims the foreground this way,
    /// even from a shell window that holds the foreground lock (Search, Start, a toast).
    /// Clicks anywhere else leave the app in front alone.
    pub fn set_click_activates(&self, rects: Vec<crate::geom::Rect>) {
        *self.shared.activates.borrow_mut() = rects;
    }

    /// Temporarily keeps keyboard focus in another app (Studio's desktop-pick session).
    /// Both the style and WM_MOUSEACTIVATE policy are needed for layered desktop popups.
    pub fn set_no_activate(&self, disabled: bool) {
        if self.shared.no_activate.replace(disabled) == disabled {
            return;
        }
        // SAFETY: changes only this owned window's activation style.
        unsafe {
            let ex = WINDOW_EX_STYLE(GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32);
            let normally_disabled = self.shared.normally_no_activate;
            let ex = if disabled || normally_disabled {
                ex | WS_EX_NOACTIVATE
            } else {
                ex & !WS_EX_NOACTIVATE
            };
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex.0 as isize);
        }
    }

    /// Makes this window the foreground window, and says how that went (for logs).
    ///
    /// A click our hook swallowed never reached any window, so Windows may not count us as the
    /// receiver of the last input and refuse a plain `SetForegroundWindow`. Then this attaches
    /// our input queue to the foreground window's thread for the call (a documented way to
    /// share its foreground right). Neither helps against a shell window holding the
    /// foreground lock: there the click itself must activate us ([`Self::set_click_activates`]).
    pub fn take_focus(&self) -> &'static str {
        if self.shared.no_activate.get() {
            return "suppressed";
        }
        let hwnd = self.hwnd;
        // SAFETY: plain window and thread queries; the queue attachment is undone at once.
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            if GetForegroundWindow() == hwnd {
                return "plain";
            }
            let current = GetCurrentThreadId();
            let owner = GetWindowThreadProcessId(GetForegroundWindow(), None);
            if owner == 0 || owner == current {
                return "refused";
            }
            let attached = AttachThreadInput(current, owner, true).as_bool();
            let _ = SetForegroundWindow(hwnd);
            if attached {
                let _ = AttachThreadInput(current, owner, false);
            }
            if GetForegroundWindow() == hwnd {
                "attached"
            } else {
                "refused"
            }
        }
    }

    fn dib_dc(&self) -> HDC {
        self.pixels
            .as_ref()
            .map_or(HDC::default(), |(_, dib)| dib.dc)
    }

    /// `UpdateLayeredWindowIndirect`: changed position and alpha, plus pixels from `source`
    /// (the DC, its dirty area, and whether the size changed) when there are new ones.
    fn update(&mut self, origin: (i32, i32), source: Option<(HDC, PixelRect, bool)>) -> Result<()> {
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: self.alpha,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let at = POINT {
            x: origin.0,
            y: origin.1,
        };
        let size = SIZE {
            cx: self.size.0 as i32,
            cy: self.size.1 as i32,
        };
        let zero = POINT::default();
        // The glass moves first (inside the engine's own handling of the request), so both
        // windows land in the same composition frame.
        if origin != self.origin
            && self.glass_open.get()
            && let (Some(glass), Some(backdrop)) = (self.glass.get(), self.backdrop())
        {
            backdrop.place(self.hwnd, glass.shape(origin), glass.radius);
        }
        let dirty = source.map(|(_, area, _)| RECT {
            left: area.x as i32,
            top: area.y as i32,
            right: (area.x + area.width) as i32,
            bottom: (area.y + area.height) as i32,
        });
        let info = UPDATELAYEREDWINDOWINFO {
            cbSize: size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
            hdcDst: HDC::default(),
            pptDst: if origin != self.origin {
                &at
            } else {
                std::ptr::null()
            },
            // Always with new pixels, resized or not: without a size, Windows accepts the call
            // and keeps showing the old pixels (measured live: every hover and press on the
            // bars and widgets stayed invisible).
            psize: if source.is_some() {
                &size
            } else {
                std::ptr::null()
            },
            hdcSrc: source.map_or(HDC::default(), |(dc, _, _)| dc),
            pptSrc: if source.is_some() {
                &zero
            } else {
                std::ptr::null()
            },
            crKey: COLORREF(0),
            pblend: &blend,
            dwFlags: ULW_ALPHA,
            // A new size needs the whole bitmap.
            prcDirty: match (&dirty, source) {
                (Some(rect), Some((_, _, false))) => rect,
                _ => std::ptr::null(),
            },
        };
        // SAFETY: every pointer in `info` refers to a local that outlives the call.
        unsafe { UpdateLayeredWindowIndirect(self.hwnd, &info) }
            .ok()
            .context("UpdateLayeredWindowIndirect")?;
        self.origin = origin;
        self.shared.origin.set(origin);
        Ok(())
    }

    /// Records `list` as on screen (hits included) without drawing it.
    pub(crate) fn set_shown(&mut self, list: DisplayList) {
        self.set_hits(&list);
        self.shown = list;
    }

    fn set_hits(&self, list: &DisplayList) {
        if let Some(app) = &self.shared.app {
            let mut hits = app.hits.borrow_mut();
            hits.clear();
            hits.extend(list.hits.iter().map(|h| h.rect));
        }
        let events = self.shared.a11y.try_borrow_mut().ok().and_then(|mut a11y| {
            a11y.as_mut()?
                .update_full(&list.hits, list.focus, &self.shared.semantics.borrow())
        });
        if let Some(events) = events {
            events.raise();
        }
    }

    /// Optional metadata for the next presented frame; callers that omit it retain their existing tree.
    pub fn set_accessibility_semantics(&self, semantics: Vec<(HitId, crate::Semantics)>) {
        *self.shared.semantics.borrow_mut() = semantics;
    }

    /// Consume a rich UIA request when handling its corresponding `Event::Invoke`.
    pub fn take_accessibility_action(&self, hit: HitId) -> Option<crate::AccessibilityAction> {
        self.shared.a11y.borrow().as_ref()?.take_action(hit)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &self.gpu {
            super::gpu_glass::unregister(self.hwnd);
            let mut gpu = gpu.borrow_mut();
            gpu.glass = None;
            gpu.layer = None;
        }
        self.close_glass();
        self.discard_events();
        // SAFETY: our own window, destroyed on its creating thread.
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Device pixels per design pixel of `hwnd` (its DPI / 96).
#[cfg(feature = "gpu")]
fn scale_of(hwnd: HWND) -> f32 {
    // SAFETY: a query on our own window.
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

fn desktop_member(window: HWND) -> bool {
    // SAFETY: read-only query of a live or recently destroyed window handle.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetPropW(window, crate::identity::desktop_prop()).0
            as usize
            == 1
            || is_desktop_glass(window)
    }
}
fn is_desktop_glass(window: HWND) -> bool {
    let mut class = [0u16; 64];
    // SAFETY: bounded local buffer for a read-only class-name query.
    let len = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClassNameW(window, &mut class) }
        .max(0) as usize;
    crate::identity::is_desktop_class(&class[..len])
}

fn register_class() -> Result<()> {
    // SAFETY: plain Win32 registration with a 'static window procedure.
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            style: CS_DBLCLKS,
            hInstance: GetModuleHandleW(None)?.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: HICON::default(),
            lpszClassName: crate::identity::surface_class(),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(windows::core::Error::from_thread()).context("RegisterClassExW");
        }
    }
    Ok(())
}

fn pointer(shared: &Shared, lparam: LPARAM) -> Pointer {
    let x = i32::from(lparam.0 as i16);
    let y = i32::from((lparam.0 >> 16) as i16);
    let (ox, oy) = shared.origin.get();
    Pointer {
        at: Point::new(x as f32, y as f32),
        screen: (ox + x, oy + y),
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA is 0 or the pointer `Surface::new` stored, valid until WM_NCDESTROY.
    let raw = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Shared;
    if raw.is_null() {
        // SAFETY: default handling of a message we were called with.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    if msg == WM_NCDESTROY {
        // SAFETY: takes back the reference the window held; nothing reads it after this.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Rc::from_raw(raw));
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
    }
    // SAFETY: the window still holds its reference (see above).
    let shared = unsafe { &*raw };
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_WINDOWPOSCHANGING
        && shared.desktop.is_some()
        && !shared.restacking.get()
        && lparam.0 != 0
    {
        // SAFETY: WM_WINDOWPOSCHANGING provides a live WINDOWPOS. Explicit desktop repairs
        // use the guard above; activation/ULW may move/resize but cannot raise a widget.
        unsafe {
            (*(lparam.0 as *mut windows::Win32::UI::WindowsAndMessaging::WINDOWPOS)).flags |=
                windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER;
        }
    }
    // The system backdrop's frost window stays directly under its surface.
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_WINDOWPOSCHANGED && lparam.0 != 0 {
        // SAFETY: WM_WINDOWPOSCHANGED provides a live WINDOWPOS.
        let flags = unsafe { (*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::WINDOWPOS)).flags };
        if !flags.contains(windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER) {
            super::glass::system().follow(hwnd);
        }
    }
    let id = shared.id;
    if let Some(app) = &shared.app
        && let Some(result) = crate::app_window::message(id, app, hwnd, msg, wparam, lparam)
    {
        return result;
    }
    if let Some(field) = shared.field.borrow().clone() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, GetFocus};
        use windows::Win32::UI::WindowsAndMessaging::{
            WM_CHAR, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_KEYUP, WM_IME_SETCONTEXT,
            WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WM_INPUTLANGCHANGEREQUEST, WM_KEYFIRST,
            WM_KEYLAST, WM_MOUSEWHEEL, WM_TIMER,
        };
        if matches!(msg, WM_INPUTLANGCHANGE | WM_INPUTLANGCHANGEREQUEST) {
            field.send(msg, wparam, lparam);
            // SAFETY: preserve the normal HWND language/IME-context update after notifying
            // the windowless services. The original message's data is still valid.
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        if msg == crate::text_field::WM_TEXT_INVALIDATED {
            field.invalidated();
            push(id, Event::TextChanged);
            return LRESULT(0);
        }
        // SAFETY: read-only queries of this thread's focus and capture windows.
        let focused = unsafe { GetFocus() } == hwnd;
        // SAFETY: same read-only query, with no borrowed pointer.
        let captured = unsafe { GetCapture() } == hwnd;
        let mouse = (WM_MOUSEMOVE..=WM_MOUSEWHEEL).contains(&msg);
        let keyboard = (WM_KEYFIRST..=WM_KEYLAST).contains(&msg)
            || matches!(
                msg,
                WM_IME_STARTCOMPOSITION | WM_IME_ENDCOMPOSITION | WM_IME_COMPOSITION
            )
            || (WM_IME_SETCONTEXT..=WM_IME_KEYUP).contains(&msg);
        let mut field_point = POINT {
            x: i32::from(lparam.0 as i16),
            y: i32::from((lparam.0 >> 16) as i16),
        };
        if msg == WM_MOUSEWHEEL {
            // SAFETY: wheel messages carry screen pixels; RichEdit's hit region is client-local.
            let _ = unsafe { ScreenToClient(hwnd, &mut field_point) };
        }
        let in_field = field.contains(field_point.x, field_point.y);
        if msg == WM_LBUTTONDOWN && in_field && !focused {
            field.focus();
        }
        // An `el` field takes the keys `el`'s policy gives it (crate::el::edit::command); the
        // rest go to the app as keys, untouched by RichEdit.
        let mut refused = false;
        // An `el` field attached while the window was inactive takes the focus with it.
        if field.is_el() && msg == WM_SETFOCUS {
            field.focus();
            return LRESULT(0);
        }
        if field.is_el() && focused {
            use crate::el::edit::{Cmd, command, types};
            use windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let (key, mods) = (crate::el::KeyCode(wparam.0 as u16), super::host::modifiers());
                match command(key, mods, field.multiline()) {
                    Some(Cmd::Submit) => {
                        push(id, Event::TextSubmit);
                        return LRESULT(0);
                    }
                    Some(_) => {}
                    None => refused = !types(key, mods),
                }
            }
            // Tab, Escape and the Enter that submitted type nothing.
            if msg == WM_CHAR && (matches!(wparam.0, 9 | 10 | 27) || wparam.0 == 13 && !field.multiline()) {
                return LRESULT(0);
            }
        }
        if msg == WM_KEYDOWN && focused && wparam.0 == 13 && !field.multiline() && !field.is_el() {
            push(id, Event::TextSubmit);
            return LRESULT(0);
        }
        // A one-line field has no use for Tab, Up, Down, Page Up or Page Down: they stay the
        // app's (moving focus, or through a list of results under a search field).
        if msg == WM_KEYDOWN
            && focused
            && !field.multiline()
            && !field.is_el()
            && matches!(wparam.0, 0x09 | 0x21 | 0x22 | 0x26 | 0x28)
        {
            push(id, Event::Key(wparam.0 as u16));
            return LRESULT(0);
        }
        if msg == WM_CHAR && focused && wparam.0 == 13 && !field.multiline() {
            return LRESULT(0);
        }
        // A single line has no use for Up, Down, Tab and the page keys: they move the
        // selection of whatever the field filters (the launcher's results).
        const NAVIGATION: [usize; 5] = [0x26, 0x28, 0x09, 0x21, 0x22];
        if focused && !field.multiline() && !field.is_el() {
            if msg == WM_KEYDOWN
                && (NAVIGATION.contains(&wparam.0)
                    || ([0x25, 0x27].contains(&wparam.0)
                        && field.text().is_ok_and(|text| text.is_empty())))
            {
                push(id, Event::Key(wparam.0 as u16));
                return LRESULT(0);
            }
            if msg == WM_CHAR && wparam.0 == 9 {
                return LRESULT(0);
            }
        }
        if msg == WM_TIMER && field.timer(wparam.0) {
            return field.send(msg, WPARAM(wparam.0 - 0x50000), lparam);
        }
        if (mouse && (in_field || captured))
            || (keyboard && focused && wparam.0 != 27 && !refused)
            || msg == WM_SETFOCUS
            || msg == WM_KILLFOCUS
        {
            let result = field.send(msg, wparam, lparam);
            if msg == WM_KILLFOCUS {
                push(id, Event::Deactivated);
            }
            return result;
        }
    }
    let button = |button| (pointer(shared, lparam), button);
    #[cfg(feature = "gpu")]
    match msg {
        // A GPU window has no layered bitmap to hit-test by alpha: what its last frame drew is
        // ours, the rest lets clicks through to the windows of this thread below.
        windows::Win32::UI::WindowsAndMessaging::WM_NCHITTEST => {
            if let Some(solid) = shared.solid.borrow().as_ref() {
                let (ox, oy) = shared.origin.get();
                let at = Point::new(
                    (i32::from(lparam.0 as i16) - ox) as f32,
                    (i32::from((lparam.0 >> 16) as i16) - oy) as f32,
                );
                const HTTRANSPARENT: isize = -1;
                const HTCLIENT: isize = 1;
                return LRESULT(if solid.iter().any(|r| r.contains(at)) { HTCLIENT } else { HTTRANSPARENT });
            }
        }
        super::gpu_glass::WM_GLASS_FRAME => {
            super::gpu_glass::tick(hwnd);
            return LRESULT(0);
        }
        _ => {}
    }
    match msg {
        WM_FRAME => push(id, Event::Frame),
        WM_MOUSEACTIVATE if shared.no_activate.get() => return LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEACTIVATE if !shared.activate => {
            // SAFETY: reads the position of the message being handled.
            let at = unsafe { windows::Win32::UI::WindowsAndMessaging::GetMessagePos() };
            let (origin_x, origin_y) = shared.origin.get();
            let at = Point::new(
                (i32::from(at as i16) - origin_x) as f32,
                (i32::from((at >> 16) as i16) - origin_y) as f32,
            );
            let activates = shared.activates.borrow().iter().any(|r| r.contains(at));
            return LRESULT(if activates {
                MA_ACTIVATE
            } else {
                MA_NOACTIVATE
            } as isize);
        }
        WM_MOUSEMOVE => {
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
            };
            // SAFETY: tracks this owned window; no callback or borrowed state escapes.
            unsafe {
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                });
            }
            push(id, Event::Move(pointer(shared, lparam)));
        }
        WM_MOUSELEAVE => push(id, Event::Leave),
        windows::Win32::UI::WindowsAndMessaging::WM_DROPFILES => {
            // SAFETY: the HDROP this message carries, read then released once.
            let Some((at, paths)) = (unsafe { dropped_files(wparam.0 as *mut _) }) else { return LRESULT(0) };
            set_dropped_files(paths);
            let (ox, oy) = shared.origin.get();
            push(id, Event::FilesDropped(Pointer { at: Point::new(at.x as f32, at.y as f32), screen: (ox + at.x, oy + at.y) }));
        }
        WM_CAPTURECHANGED => push(id, Event::CaptureLost),
        WM_MOUSEWHEEL => {
            let mut at = POINT {
                x: i32::from(lparam.0 as i16),
                y: i32::from((lparam.0 >> 16) as i16),
            };
            let screen = (at.x, at.y);
            // SAFETY: owned window and writable local point.
            let _ = unsafe { ScreenToClient(hwnd, &mut at) };
            push(
                id,
                Event::Wheel(
                    Pointer {
                        at: Point::new(at.x as f32, at.y as f32),
                        screen,
                    },
                    (wparam.0 >> 16) as i16,
                ),
            );
        }
        WM_LBUTTONDBLCLK | WM_RBUTTONDBLCLK | WM_MBUTTONDBLCLK => {
            let (p, b) = button(button_of(msg));
            push(id, Event::Press(p, b));
            push(id, Event::DoubleClick(p, b));
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
            let (p, b) = button(button_of(msg));
            push(id, Event::Press(p, b));
        }
        WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
            let (p, b) = button(button_of(msg));
            push(id, Event::Release(p, b));
        }
        WM_KEYDOWN => push(id, Event::Key(wparam.0 as u16)),
        windows::Win32::UI::WindowsAndMessaging::WM_KEYUP
        | windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYUP => push(id, Event::KeyUp(wparam.0 as u16)),
        windows::Win32::UI::WindowsAndMessaging::WM_DPICHANGED => {
            push(id, Event::DpiChanged((wparam.0 & 0xffff) as u32))
        }
        windows::Win32::UI::WindowsAndMessaging::WM_SHOWWINDOW => {
            push(id, Event::VisibilityChanged)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_WINDOWPOSCHANGED
            if shared.desktop.is_some() && !shared.restacking.get() =>
        {
            push(id, Event::VisibilityChanged)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_CANCELMODE => push(id, Event::Cancelled),
        WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE => push(id, Event::Deactivated),
        WM_ACTIVATE => push(id, Event::Activated),
        WM_A11Y_ACTION => {
            let hit = lparam.0 as HitId;
            push(
                id,
                if wparam.0 == 0 {
                    Event::Invoke(hit)
                } else {
                    Event::Focus(hit)
                },
            );
            return LRESULT(0);
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            let events = shared
                .a11y
                .try_borrow_mut()
                .ok()
                .and_then(|mut a| a.as_mut()?.focus_changed(msg == WM_SETFOCUS));
            if let Some(events) = events {
                events.raise();
            }
        }
        WM_GETOBJECT => {
            let answer = shared
                .a11y
                .try_borrow_mut()
                .ok()
                .and_then(|mut a| a.as_mut()?.get_object(wparam, lparam));
            if let Some(answer) = answer {
                return answer.into();
            }
        }
        _ => {}
    }
    // SAFETY: default handling of a message we were called with.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn button_of(msg: u32) -> Button {
    match msg {
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK => Button::Left,
        WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK => Button::Right,
        _ => Button::Middle,
    }
}

impl crate::platform::Window for Surface {
    type Bitmap = HDC;
    fn new(kind: Kind, name: &str, role: Role) -> Result<Self> { Self::new(kind, name, role) }
    fn present_bitmap(&mut self, origin: (i32, i32), bitmap: Option<(&HDC, PixelRect, bool)>) -> Result<()> {
        self.update(origin, bitmap.map(|(dc, dirty, resized)| (*dc, dirty, resized)))
    }
    fn next_event() -> Option<(SurfaceId, Event)> { next_event() }
}

/// The thread's default render mode; always `Cpu` without the `gpu` feature (nothing is read).
fn default_render() -> crate::platform::Render {
    #[cfg(feature = "gpu")]
    return crate::platform::render();
    #[cfg(not(feature = "gpu"))]
    crate::platform::Render::Cpu
}

/// Native layers by (parent window, name): the child window and where it was last placed.
type Natives = std::collections::HashMap<(isize, String), (isize, Option<crate::Rect>)>;

thread_local! {
    static NATIVES: RefCell<Natives> = RefCell::new(Natives::new());
}

/// A native layer's window procedure: transparent to hit testing, so the pointer reaches the
/// surface (which routes it like any other input) and its DirectComposition content on top.
unsafe extern "system" fn native_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{HTTRANSPARENT, WM_NCHITTEST};
    if msg == WM_NCHITTEST {
        return LRESULT(HTTRANSPARENT as isize);
    }
    // SAFETY: the default procedure for this window's own messages.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

impl Surface {
    /// The native layer `name`, made on first use: a child window under the surface's content
    /// where the host presents with its own GPU device (a Vulkan or DXGI swapchain for the
    /// child's client area). It shows where the content is transparent ([`crate::Op::Hole`],
    /// drawn by [`crate::el::native`]) once placed. The surface must render on the GPU
    /// ([`Render::Gpu`](crate::platform::Render::Gpu), the `gpu` feature): its
    /// DirectComposition content is composed over child windows; a CPU (layered) surface can't
    /// show one, and this fails.
    pub fn native_layer(&self, name: &str) -> Result<crate::NativeHandle> {
        use windows::Win32::UI::WindowsAndMessaging::{WS_CHILD, WS_CLIPSIBLINGS};
        use windows::core::w;
        let parent = self.hwnd.0 as isize;
        if let Some(child) = NATIVES.with_borrow(|n| n.get(&(parent, name.to_owned())).map(|(c, _)| *c)) {
            return Ok(crate::NativeHandle::Hwnd(child));
        }
        anyhow::ensure!(
            self.render() == crate::platform::Render::Gpu,
            "native layers need a GPU surface on Windows (Render::Gpu): a layered CPU window can't show child windows"
        );
        // SAFETY: plain Win32 class registration and child window creation on this thread.
        let child = unsafe {
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(native_proc),
                hInstance: GetModuleHandleW(None)?.into(),
                lpszClassName: w!("StillkitNativeLayer"),
                ..Default::default()
            };
            if RegisterClassExW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(windows::core::Error::from_thread()).context("RegisterClassExW");
            }
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("StillkitNativeLayer"),
                w!(""),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                Some(self.hwnd),
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        let child = child.0 as isize;
        NATIVES.with_borrow_mut(|n| n.insert((parent, name.to_owned()), (child, None)));
        Ok(crate::NativeHandle::Hwnd(child))
    }

    /// Shows native layer `name` at `rect` (device px from the client's top-left), or hides it
    /// (`None`). Unchanged rects cost nothing; unknown names are ignored.
    pub fn place_native_layer(&self, name: &str, rect: Option<crate::Rect>) {
        use windows::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SW_SHOWNA, SetWindowPos};
        let key = (self.hwnd.0 as isize, name.to_owned());
        let Some(child) = NATIVES.with_borrow_mut(|n| {
            let entry = n.get_mut(&key)?;
            (entry.1 != rect).then(|| {
                entry.1 = rect;
                entry.0
            })
        }) else {
            return;
        };
        let child = HWND(child as *mut _);
        // SAFETY: a live child window of this surface.
        unsafe {
            match rect {
                Some(r) => {
                    let _ = SetWindowPos(
                        child,
                        None,
                        r.left.round() as i32,
                        r.top.round() as i32,
                        r.width().round().max(1.0) as i32,
                        r.height().round().max(1.0) as i32,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let _ = ShowWindow(child, SW_SHOWNA);
                }
                None => {
                    let _ = ShowWindow(child, SW_HIDE);
                }
            }
        }
    }

    /// Removes native layer `name`; its handle is invalid afterwards.
    pub fn release_native_layer(&self, name: &str) {
        if let Some((child, _)) = NATIVES.with_borrow_mut(|n| n.remove(&(self.hwnd.0 as isize, name.to_owned()))) {
            // SAFETY: a child window this surface made.
            let _ = unsafe { DestroyWindow(HWND(child as *mut _)) };
        }
    }
}

thread_local! {
    static DROPPED: std::cell::RefCell<Vec<std::path::PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The paths of the files dropped with the last [`Event::FilesDropped`] (taken once).
pub fn take_dropped_files() -> Vec<std::path::PathBuf> {
    DROPPED.with_borrow_mut(std::mem::take)
}

/// Reads and releases a `WM_DROPFILES` drop: where it landed (client px) and its paths.
/// shell32.dll's drop functions are looked up on the first drop, not imported: a static import
/// would map shell32 into every stillkit process at start (rule: nothing loads until asked).
///
/// # Safety
/// `drop` is the `HDROP` a `WM_DROPFILES` message carried, not yet finished.
unsafe fn dropped_files(drop: *mut core::ffi::c_void) -> Option<(POINT, Vec<std::path::PathBuf>)> {
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW};
    use windows::core::{s, w};
    type QueryFile = unsafe extern "system" fn(*mut core::ffi::c_void, u32, *mut u16, u32) -> u32;
    type QueryPoint = unsafe extern "system" fn(*mut core::ffi::c_void, *mut POINT) -> i32;
    type Finish = unsafe extern "system" fn(*mut core::ffi::c_void);
    // SAFETY: system32's shell32 and its documented exports, called with their signatures on
    // the caller's live HDROP, which DragFinish releases last.
    unsafe {
        let shell = LoadLibraryExW(w!("shell32.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
        let query: QueryFile = std::mem::transmute(GetProcAddress(shell, s!("DragQueryFileW"))?);
        let point: QueryPoint = std::mem::transmute(GetProcAddress(shell, s!("DragQueryPoint"))?);
        let finish: Finish = std::mem::transmute(GetProcAddress(shell, s!("DragFinish"))?);
        let mut at = POINT::default();
        point(drop, &mut at);
        let paths = (0..query(drop, u32::MAX, std::ptr::null_mut(), 0))
            .map(|i| {
                let mut buf = vec![0u16; query(drop, i, std::ptr::null_mut(), 0) as usize + 1];
                let len = query(drop, i, buf.as_mut_ptr(), buf.len() as u32) as usize;
                std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len]))
            })
            .collect();
        finish(drop);
        Some((at, paths))
    }
}

/// Keeps dropped paths for [`take_dropped_files`].
#[allow(dead_code)]
pub(super) fn set_dropped_files(paths: Vec<std::path::PathBuf>) {
    DROPPED.with_borrow_mut(|d| *d = paths);
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

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalUnlock};

    /// A drop as Explorer builds it: a `DROPFILES` header, then nul-separated wide paths.
    #[test]
    fn a_drop_reads_its_point_and_paths_through_the_lazy_shell32_calls() {
        let paths = [r"C:\one.txt", r"D:\a folder\two"];
        let mut wide: Vec<u16> = paths.iter().flat_map(|p| p.encode_utf16().chain([0])).collect();
        wide.push(0);
        const HEADER: usize = 20; // pFiles, pt, fNC, fWide
        // SAFETY: a fresh allocation sized for the header and paths, filled while locked; the
        // reader's DragFinish frees it.
        let drop = unsafe {
            let global = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, HEADER + wide.len() * 2).unwrap();
            let bytes = GlobalLock(global).cast::<u8>();
            bytes.cast::<u32>().write_unaligned(HEADER as u32);
            bytes.add(4).cast::<POINT>().write_unaligned(POINT { x: 12, y: 34 });
            bytes.add(16).cast::<i32>().write_unaligned(1);
            std::ptr::copy_nonoverlapping(wide.as_ptr().cast::<u8>(), bytes.add(HEADER), wide.len() * 2);
            let _ = GlobalUnlock(global);
            global.0
        };
        // SAFETY: an unfinished HDROP made above.
        let (at, read) = unsafe { super::dropped_files(drop) }.expect("shell32's drop functions");
        assert_eq!((at.x, at.y), (12, 34));
        assert_eq!(read, paths.map(std::path::PathBuf::from));
    }
}
