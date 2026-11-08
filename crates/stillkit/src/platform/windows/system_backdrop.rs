//! The built-in CPU-mode backdrop (docs/ui.md, "System backdrop"): DWM's own host backdrop blur
//! (the acrylic blur of what is behind a window) under a surface's body, through
//! Windows.UI.Composition. DWM draws the blur out of process, so no GPU device or vendor driver
//! loads here (`tests/no_gpu_driver.rs`).
//!
//! Each surface with glass gets a frost window directly under it, the size of its body: layered
//! with a fully transparent bitmap (click-through, and moved by a position-only
//! `UpdateLayeredWindow`), never activated, topmost when the surface is. Its
//! [`DesktopWindowTarget`] holds one visual clipped to the body's rounded rect: the blur and, over
//! it, the material's tint. It is placed before every move of its surface (as the engine's glass
//! windows are), and put back under it when the surface's z-order changes
//! ([`SystemBackdrop::follow`]). The frost can't live in the surface's own window: DWM composes a
//! layered window's visuals over its pixels whatever the target's `isTopmost` says (measured,
//! `backdrop_spike layered`), and the blur is opaque.
//!
//! Nothing is made until the first surface opens glass, and the windows, visuals and compositor
//! all go when the last one closes. What it can't do: lenses and rims (a lensed material gets the
//! frost alone), liquid merging, choosing the blur radius (the system's), and app windows (they
//! are opaque on the CPU). It needs Windows 11, Settings' "Transparency effects", and a session
//! where a compositor and a desktop window target can actually be made (probed once per thread);
//! without any of them, [`available`](Backdrop::available) is false, nothing opens, and views draw
//! their fill.
//!
//! Every UI thread has its own: the provider, its dispatcher queue, its compositor (activated from
//! a factory fetched on that thread, never a process-wide cached one: [`compositor`]).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use anyhow::{Context, Result, bail};
use windows::System::DispatcherQueueController;
use windows::UI::Color;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionColorBrush, Compositor, ContainerVisual, SpriteVisual};
use windows::Win32::Foundation::{CO_E_NOTINITIALIZED, COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_HOSTBACKDROPBRUSH, DwmSetWindowAttribute};
use windows::Win32::System::Com::CoIncrementMTAUsage;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::System::WinRT::{
    DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
    IActivationFactory, RoGetActivationFactory,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDNEXT, GWL_EXSTYLE, GetWindow,
    GetWindowLongPtrW, HTTRANSPARENT,
    MA_NOACTIVATE, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowPos,
    ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WINDOW_EX_STYLE, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{BOOL, HSTRING, Interface, RuntimeName, w};
use windows_numerics::Vector2;

use super::glass::Backdrop;
use crate::glass::{GlassMaterial, Shape};

/// `CreateDispatcherQueueController`, looked up the first time a frost needs a queue. A static
/// import would map coremessaging.dll into every stillkit process at start, frost or not (rule:
/// nothing loads until a surface asks for it).
fn create_queue(options: DispatcherQueueOptions) -> Result<DispatcherQueueController> {
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW};
    type Create = unsafe extern "system" fn(DispatcherQueueOptions, *mut *mut core::ffi::c_void) -> windows::core::HRESULT;
    // SAFETY: a System32 DLL by name; the handle stays loaded for the process's life.
    let module = unsafe { LoadLibraryExW(w!("coremessaging.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
        .context("coremessaging.dll")?;
    // SAFETY: a live module and a NUL-terminated export name.
    let proc = unsafe { GetProcAddress(module, windows::core::s!("CreateDispatcherQueueController")) }
        .context("CreateDispatcherQueueController")?;
    // SAFETY: the export has exactly this signature (dispatcherqueue.h).
    let create: Create = unsafe { std::mem::transmute(proc) };
    let mut raw = std::ptr::null_mut();
    // SAFETY: `options` is plain data; `raw` receives an owned reference on success.
    unsafe { create(options, &mut raw) }.ok()?;
    // SAFETY: on success `raw` is a DispatcherQueueController we now own.
    Ok(unsafe { DispatcherQueueController::from_raw(raw) })
}

/// The provider of one UI thread ([`crate::glass`] uses it when no backdrop is installed).
#[derive(Default)]
pub struct SystemBackdrop {
    /// Made on the first open, dropped when the last frost closes.
    live: RefCell<Option<Live>>,
    /// Whether this thread can make frost: `None` until probed ([`Self::usable`]); `false` from
    /// then on after a failed probe or open.
    usable: Cell<Option<bool>>,
    /// A step made to fail ([`Self::fail_at`]).
    fault: Cell<Option<Step>>,
}

/// A step of making frost, for [`SystemBackdrop::fail_at`].
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The thread's dispatcher queue.
    Queue,
    /// The compositor.
    Compositor,
    /// A frost window.
    Window,
    /// Its desktop window target.
    Target,
    /// The host backdrop brush.
    Brush,
    /// Placing an open frost (not probed: only an open reaches it).
    Place,
}

struct Live {
    /// By client window. Before the compositor: fields drop in order, so the targets and visuals
    /// go before the compositor that made them.
    frosts: HashMap<isize, Frost>,
    compositor: Compositor,
}

/// One surface's frost. Kept, hidden, while closed and others are open.
struct Frost {
    /// Before the window: the target and visuals are released while their window still exists.
    visuals: Visuals,
    window: Window,
    /// The clip made last: width, height and radius.
    clip: Cell<Option<(i32, i32, f32)>>,
    open: bool,
}

struct Visuals {
    _target: DesktopWindowTarget,
    body: ContainerVisual,
    blur: SpriteVisual,
    tint: CompositionColorBrush,
}

/// A frost window, destroyed with this.
struct Window(HWND);

thread_local! {
    /// The thread's dispatcher queue, which Windows.UI.Composition needs. Made once and kept:
    /// it is a few kilobytes, and a thread can't make a second one while the first shuts down.
    static QUEUE: RefCell<Option<DispatcherQueueController>> = const { RefCell::new(None) };
}

impl SystemBackdrop {
    /// Whether any frost is open (the compositor is up).
    pub fn active(&self) -> bool {
        self.live.try_borrow().is_ok_and(|live| live.is_some())
    }

    /// The frost window under `client`, while one is open.
    pub fn window(&self, client: HWND) -> Option<HWND> {
        // A re-entrant ask (a message sent while an open or close holds `live`) finds none.
        let live = self.live.try_borrow().ok()?;
        live.as_ref()?.frosts.get(&(client.0 as isize)).filter(|f| f.open).map(|f| f.window.0)
    }

    /// Puts `client`'s frost back directly under it (its z-order changed).
    pub fn follow(&self, client: HWND) {
        if let Some(window) = self.window(client)
            && !directly_under(window, client)
        {
            // SAFETY: two windows of this thread; z-order only.
            let _ = unsafe { SetWindowPos(window, Some(client), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        }
    }

    /// Makes `step` fail on this thread from now on (`None`: nothing fails), as it would in a
    /// session without usable composition, and forgets the last probe. For tests.
    #[doc(hidden)]
    pub fn fail_at(&self, step: Option<Step>) {
        self.fault.set(step);
        self.usable.set(None);
    }

    fn check(&self, step: Step) -> Result<()> {
        if self.fault.get() == Some(step) {
            bail!("{step:?} failed (injected)");
        }
        Ok(())
    }

    /// Whether this thread can make frost, found once by making it: the queue, a compositor, and
    /// a hidden frost window with its target and blur, all dropped again. Only asked on Windows
    /// 11, so older systems make nothing.
    fn usable(&self) -> bool {
        if let Some(usable) = self.usable.get() {
            return usable;
        }
        let probe = || -> Result<()> {
            self.ensure_queue()?;
            let compositor = self.compositor()?;
            drop(Frost::new(self, &compositor, false)?);
            Ok(())
        };
        let usable = match probe() {
            Ok(()) => true,
            Err(e) => {
                log::info!("system backdrop unavailable: {e:#}");
                false
            }
        };
        self.usable.set(Some(usable));
        usable
    }

    fn try_open(&self, client: HWND, shape: Shape, radius: f32, material: &GlassMaterial) -> Result<()> {
        if !self.available() {
            bail!("unavailable");
        }
        let mut slot = self.live.try_borrow_mut().context("re-entered")?;
        let live = match slot.take() {
            Some(live) => live,
            None => {
                self.ensure_queue()?;
                Live { frosts: HashMap::new(), compositor: self.compositor()? }
            }
        };
        let live = slot.insert(live);
        let frost = match live.frosts.entry(client.0 as isize) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                // SAFETY: a read of our own window's style.
                let topmost = unsafe { WINDOW_EX_STYLE(GetWindowLongPtrW(client, GWL_EXSTYLE) as u32) }.contains(WS_EX_TOPMOST);
                let frost = Frost::new(self, &live.compositor, topmost)?;
                frost.mark_desktop(client);
                entry.insert(frost)
            }
        };
        frost.material(material)?;
        self.check(Step::Place)?;
        frost.place(&live.compositor, client, shape, radius)?;
        // SAFETY: our own window, shown without activation.
        let _ = unsafe { ShowWindow(frost.window.0, SW_SHOWNOACTIVATE) };
        frost.open = true;
        Ok(())
    }

    fn ensure_queue(&self) -> Result<()> {
        self.check(Step::Queue)?;
        if windows::System::DispatcherQueue::GetForCurrentThread().is_ok() {
            return Ok(());
        }
        QUEUE.with_borrow_mut(|queue| {
            if queue.is_none() {
                let options = DispatcherQueueOptions {
                    dwSize: size_of::<DispatcherQueueOptions>() as u32,
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_NONE,
                };
                *queue = Some(create_queue(options).context("dispatcher queue")?);
            }
            Ok(())
        })
    }

    /// A compositor for this thread, activated from a factory fetched here and now.
    ///
    /// Not `Compositor::new()`: windows-rs caches an activation factory for the whole process
    /// once it reports `IAgileObject`, which the compositor's does. After the thread that first
    /// used it for a desktop window target exits, activating a compositor from that cached factory
    /// on another thread is an access violation (`tests/backdrop_system.rs`,
    /// `a_thread_after_a_finished_frost_thread_gets_its_own`).
    fn compositor(&self) -> Result<Compositor> {
        self.check(Step::Compositor)?;
        let name = HSTRING::from(Compositor::NAME);
        // SAFETY: plain activation calls with a valid class name.
        unsafe {
            let factory: IActivationFactory = match RoGetActivationFactory(&name) {
                // No apartment on this thread (the queue is made with `DQTAT_COM_NONE`): join the
                // process's implicit MTA, as windows-rs does, and keep it for the process.
                Err(e) if e.code() == CO_E_NOTINITIALIZED => {
                    CoIncrementMTAUsage().context("joining the MTA")?;
                    RoGetActivationFactory(&name)
                }
                other => other,
            }
            .context("compositor factory")?;
            factory.ActivateInstance()?.cast().context("Compositor")
        }
    }
}

impl Backdrop for SystemBackdrop {
    fn available(&self) -> bool {
        supported() && transparency() && self.usable()
    }

    fn open(&self, client: HWND, shape: Shape, radius: f32, material: &GlassMaterial) -> bool {
        match self.try_open(client, shape, radius, material) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("system backdrop: {e:#}");
                // Fail closed: views draw their fill from now on.
                self.usable.set(Some(false));
                self.close(client);
                false
            }
        }
    }

    fn place(&self, client: HWND, shape: Shape, radius: f32) {
        let Ok(live) = self.live.try_borrow() else { return };
        let Some(live) = live.as_ref() else { return };
        if let Some(frost) = live.frosts.get(&(client.0 as isize))
            && let Err(e) = frost.place(&live.compositor, client, shape, radius)
        {
            log::warn!("system backdrop: {e:#}");
        }
    }

    fn material(&self, client: HWND, material: &GlassMaterial) {
        let Ok(live) = self.live.try_borrow() else { return };
        if let Some(frost) = live.as_ref().and_then(|l| l.frosts.get(&(client.0 as isize)))
            && let Err(e) = frost.material(material)
        {
            log::warn!("system backdrop: {e:#}");
        }
    }

    fn close(&self, client: HWND) {
        let Ok(mut live) = self.live.try_borrow_mut() else { return };
        let Some(l) = live.as_mut() else { return };
        if let Some(frost) = l.frosts.get_mut(&(client.0 as isize)) {
            // SAFETY: our own window.
            let _ = unsafe { ShowWindow(frost.window.0, SW_HIDE) };
            frost.open = false;
        }
        // The last one closed: the windows, visuals and compositor all go.
        if l.frosts.values().all(|f| !f.open) {
            *live = None;
        }
    }
}

impl Frost {
    /// A hidden frost window with its target and visuals; nothing is left behind on failure.
    fn new(backdrop: &SystemBackdrop, compositor: &Compositor, topmost: bool) -> Result<Self> {
        backdrop.check(Step::Window)?;
        let window = create_window(topmost)?;
        let visuals = Visuals::new(backdrop, compositor, window.0)?;
        Ok(Self { visuals, window, clip: Cell::new(None), open: false })
    }

    /// A desktop surface's frost belongs to its desktop band: marked as the surface is, so the
    /// band's order checks and repairs (`keep_above_desktop`) step over it.
    fn mark_desktop(&self, client: HWND) {
        // SAFETY: property reads and writes on windows of this thread.
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{GetPropW, SetPropW};
            let mark = GetPropW(client, crate::identity::desktop_prop());
            if !mark.is_invalid() {
                let _ = SetPropW(self.window.0, crate::identity::desktop_prop(), Some(mark));
            }
        }
    }

    /// Moves the frost window to `shape` (screen px), directly under `client`, and clips it to
    /// the rounded body. A move of the same size is a position-only `UpdateLayeredWindow`,
    /// 0.2 ms against 0.5 ms for `SetWindowPos` (`backdrop_spike movebench`); a new size gives
    /// the window a transparent bitmap of that size first, and remakes the clip.
    fn place(&self, compositor: &Compositor, client: HWND, shape: Shape, radius: f32) -> Result<()> {
        let (w, h) = (shape.width as f32, shape.height as f32);
        let r = radius.clamp(0.0, w.min(h) / 2.0);
        let resized = self.clip.get().is_none_or(|(cw, ch, _)| (cw, ch) != (shape.width, shape.height));
        if self.clip.get() != Some((shape.width, shape.height, r)) {
            let corner = Vector2 { X: r, Y: r };
            self.visuals.body.SetClip(&compositor.CreateRectangleClipWithSidesAndRadius(0.0, 0.0, w, h, corner, corner, corner, corner)?)?;
            self.clip.set(Some((shape.width, shape.height, r)));
        }
        let window = self.window.0;
        let at = POINT { x: shape.x, y: shape.y };
        if resized {
            // A new size: a fully transparent layered bitmap of that size, so later moves can be
            // position-only.
            transparent(window, at, (shape.width, shape.height))?;
        } else {
            // SAFETY: our own layered window; only its position changes.
            unsafe { UpdateLayeredWindow(window, None, Some(&at), None, None, None, COLORREF(0), Some(&BLEND), ULW_ALPHA) }
                .context("moving the frost window")?;
        }
        if !directly_under(window, client) {
            // SAFETY: two windows of this thread; z-order only.
            unsafe { SetWindowPos(window, Some(client), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) }?;
        }
        Ok(())
    }

    /// The tint over the frost, as it is given (its alpha is the stain's strength). No frost at
    /// all (`blur` under half a pixel) is clear glass: the tint alone over what is behind.
    fn material(&self, material: &GlassMaterial) -> Result<()> {
        let t = material.tint;
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.visuals.tint.SetColor(Color { A: byte(t.a), R: byte(t.r), G: byte(t.g), B: byte(t.b) })?;
        self.visuals.blur.SetIsVisible(material.blur >= 0.5)?;
        Ok(())
    }
}

impl Visuals {
    fn new(backdrop: &SystemBackdrop, compositor: &Compositor, window: HWND) -> Result<Self> {
        let enable = BOOL::from(true);
        // SAFETY: our own window; the attribute is a BOOL of the size given.
        unsafe {
            DwmSetWindowAttribute(window, DWMWA_USE_HOSTBACKDROPBRUSH, (&raw const enable).cast(), size_of::<BOOL>() as u32)
        }
        .context("DWMWA_USE_HOSTBACKDROPBRUSH")?;
        backdrop.check(Step::Target)?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        // SAFETY: a live window of this thread, which also owns the compositor.
        let target = unsafe { interop.CreateDesktopWindowTarget(window, true) }.context("desktop window target")?;
        let body = compositor.CreateContainerVisual()?;
        body.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        backdrop.check(Step::Brush)?;
        let blur = compositor.CreateSpriteVisual()?;
        blur.SetBrush(&compositor.CreateHostBackdropBrush().context("host backdrop brush")?)?;
        blur.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        let tint = compositor.CreateColorBrush()?;
        let stain = compositor.CreateSpriteVisual()?;
        stain.SetBrush(&tint)?;
        stain.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        let children = body.Children()?;
        children.InsertAtTop(&blur)?;
        children.InsertAtTop(&stain)?;
        target.SetRoot(&body)?;
        Ok(Self { _target: target, body, blur, tint })
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: our own window, on its thread.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

unsafe extern "system" fn frost_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Never hit or activated: clicks belong to the surface above, or pass through.
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        // SAFETY: default handling of the message we were called with.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// A hidden frost window: composition only, click-through (layered and transparent).
fn create_window(topmost: bool) -> Result<Window> {
    let class = w!("stillkit.frost");
    // SAFETY: plain Win32 registration (a second one fails harmlessly) and creation on this thread.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(frost_proc), hInstance: instance.into(), lpszClassName: class, ..Default::default() });
        let mut ex = WS_EX_NOREDIRECTIONBITMAP | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        if topmost {
            ex |= WS_EX_TOPMOST;
        }
        CreateWindowExW(ex, class, w!("frost"), WS_POPUP, 0, 0, 1, 1, None, None, Some(instance.into()), None)
            .map(Window)
            .context("frost window")
    }
}

/// Per-pixel alpha, the bitmap as it is (a transparent one).
const BLEND: windows::Win32::Graphics::Gdi::BLENDFUNCTION = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
    BlendOp: windows::Win32::Graphics::Gdi::AC_SRC_OVER as u8,
    BlendFlags: 0,
    SourceConstantAlpha: 255,
    AlphaFormat: windows::Win32::Graphics::Gdi::AC_SRC_ALPHA as u8,
};

/// Gives the layered `window` a fully transparent bitmap of `size` at `at`. The bitmap is freed
/// at once: DWM keeps its own copy, out of this process.
fn transparent(window: HWND, at: POINT, size: (i32, i32)) -> Result<()> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, SelectObject,
    };
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size.0,
            biHeight: -size.1,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: GDI objects made and released here; a new DIB section is zeroed (transparent).
    unsafe {
        let dc = CreateCompatibleDC(None);
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        let result = bitmap.as_ref().map_err(Clone::clone).and_then(|&bitmap| {
            let old = SelectObject(dc, bitmap.into());
            let r = UpdateLayeredWindow(
                window,
                None,
                Some(&at),
                Some(&SIZE { cx: size.0, cy: size.1 }),
                Some(dc),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&BLEND),
                ULW_ALPHA,
            );
            SelectObject(dc, old);
            r
        });
        if let Ok(bitmap) = bitmap {
            let _ = DeleteObject(bitmap.into());
        }
        let _ = DeleteDC(dc);
        result.context("sizing the frost window")
    }
}

fn directly_under(window: HWND, client: HWND) -> bool {
    // SAFETY: a z-order query.
    unsafe { GetWindow(client, GW_HWNDNEXT) }.is_ok_and(|next| next == window)
}

/// Windows 11 (build 22000): host backdrop brushes for Win32 windows and rounded rectangle clips.
fn supported() -> bool {
    static BUILD: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *BUILD.get_or_init(|| {
        let mut text = [0u16; 16];
        let mut len = size_of_val(&text) as u32;
        // SAFETY: a string value into a buffer of the stated byte length.
        let ok = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
                w!("CurrentBuildNumber"),
                RRF_RT_REG_SZ,
                None,
                Some(text.as_mut_ptr().cast()),
                Some(&mut len),
            )
        };
        if ok.is_err() {
            return 0;
        }
        let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        String::from_utf16_lossy(&text[..end]).parse().unwrap_or(0)
    }) >= 22000
}

/// Settings › Personalization › Colors › Transparency effects. Read on every call: views ask
/// while they build a frame, and it is one registry read.
fn transparency() -> bool {
    let mut value = 1u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: a DWORD value into a local of the stated size.
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("EnableTransparency"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut len),
        )
    };
    ok.is_err() || value != 0
}
