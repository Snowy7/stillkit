//! Surfaces: AppKit windows whose content view's layer shows the surface's CoreGraphics bitmap
//! (`CALayer.contents`, a `CGImage`), redrawn only where the display list changed.
//!
//! - `App`: an NSWindow with a full-size content view and a transparent title bar (the app
//!   draws its own; the traffic lights stay), resizable, in the Dock and the window list.
//! - `Popup`: a borderless non-activating NSPanel at the pop-up menu level; it takes the key
//!   focus only with `activate` and when asked ([`Surface::take_focus`]).
//! - `Bar`: a borderless non-activating NSPanel at the status level on every Space, beside
//!   full-screen apps too, never activated.
//! - `DesktopChild`: a borderless window just above the desktop picture (under the icons and
//!   every app), on every Space. `parent` has no meaning here.
//!
//! All but `App` are transparent with per-pixel alpha and no system shadow; clicks pass
//! through fully transparent pixels, and `set_click_through` lets them through everywhere.
use super::app_window::{Chrome, Cursor};
use super::render::Canvas;
use super::view::View;
use crate::{DisplayList, HitId, Point, Ui};
use anyhow::{Context, Result, bail};
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBackingStoreType, NSColor, NSPanel, NSResponder, NSView, NSViewLayerContentsRedrawPolicy,
    NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask, NSWindowTitleVisibility, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
};
use objc2_core_graphics::{CGImage, CGWindowLevelForKey, CGWindowLevelKey};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::kCAGravityTopLeft;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

/// What kind of window a surface is (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Popup.
    Popup { #[doc = "Whether the popup requests activation."] activate: bool },
    /// Bar.
    Bar,
    /// Desktop child.
    DesktopChild { #[doc = "Native parent handle retained for desktop surface compatibility."] parent: usize },
    /// `min` is its smallest client size in design pixels (points).
    App { #[doc = "Minimum logical window size."] min: (u32, u32) },
}

/// Identifies a surface in [`Event`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(isize);

impl SurfaceId {
    /// The id a [`Backdrop`](super::glass::Backdrop) is told (`client`).
    pub(super) fn from_raw(raw: usize) -> Self {
        Self(raw as isize)
    }
}

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
    /// In screen pixels (from the primary screen's top-left).
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
    /// Positive delta scrolls up; 120 a wheel notch.
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
    /// The surface's window became the key window.
    Activated,
    /// The surface's window stopped being the key window.
    Deactivated,
    /// Assistive technology invoked a hit item.
    Invoke(HitId),
    /// Assistive technology moved the focus to a hit item.
    Focus(HitId),
    /// The window's backing scale changed: 96 per point-to-pixel factor (192 on Retina).
    DpiChanged(u32),
    /// Visibility changed.
    VisibilityChanged,
    /// Cancelled.
    Cancelled,
    /// A focused `TextField` changed.
    TextChanged,
    /// Return in a single-line `TextField`.
    TextSubmit,
    /// An app window's content is now this many pixels.
    Resized(u32, u32),
    /// The user asked an app window to close.
    Close,
    /// The appearance (light or dark) or an accessibility display option changed.
    SettingChanged,
    /// A character typed (a key's text, a dead-key sequence, an input method's commit), for
    /// `el`'s text fields.
    Char(char),
    /// The input method's composition changed (`ime::preedit` has it).
    Preedit,
}

pub use super::pump::next_event;

/// Queues `event` for surface `id` (its [`SurfaceId`] as a number).
pub fn queue_event(id: usize, event: Event) {
    super::pump::push(SurfaceId(id as isize), event);
}

/// Display-rate `Event::Frame`s for a surface until dropped or the duration passes.
pub struct AnimationFrames(Rc<Cell<bool>>);

impl Drop for AnimationFrames {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

thread_local! {
    static NEXT: Cell<isize> = const { Cell::new(1) };
    static VIEWS: RefCell<HashMap<SurfaceId, Weak<View>>> = RefCell::new(HashMap::new());
}

/// The live view of surface `id`.
pub(super) fn view(id: SurfaceId) -> Option<Retained<View>> {
    VIEWS.with_borrow(|v| v.get(&id).and_then(Weak::load))
}

define_class!(
    // SAFETY: NSPanel has no subclassing requirements; `Panel` doesn't implement Drop.
    #[unsafe(super(NSPanel, NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "StillkitPanel"]
    #[ivars = Cell<bool>]
    struct Panel;

    impl Panel {
        /// Borderless windows refuse the key focus; an activating popup takes it.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            self.ivars().get()
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }
    }
);

/// A borderless non-activating panel.
fn panel(mtm: MainThreadMarker, activate: bool) -> Retained<NSWindow> {
    let this = Panel::alloc(mtm).set_ivars(Cell::new(activate));
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0));
    // SAFETY: NSWindow's designated initializer.
    let panel: Retained<Panel> =
        unsafe { msg_send![super(this), initWithContentRect: rect, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false] };
    Retained::into_super(Retained::into_super(panel))
}

/// The window for `kind`, transparent unless it's an app window.
fn window(mtm: MainThreadMarker, kind: Kind, name: &str) -> Retained<NSWindow> {
    let window = match kind {
        Kind::App { min } => {
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::FullSizeContentView;
            let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(min.0.max(1) as f64, min.1.max(1) as f64));
            // SAFETY: NSWindow's designated initializer; released-when-closed is turned off below.
            let w = unsafe {
                NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), rect, style, NSBackingStoreType::Buffered, false)
            };
            w.setTitlebarAppearsTransparent(true);
            w.setTitleVisibility(NSWindowTitleVisibility::Hidden);
            w.setContentMinSize(NSSize::new(min.0 as f64, min.1 as f64));
            w
        }
        Kind::Popup { activate } => {
            let w = panel(mtm, activate);
            w.setLevel(objc2_app_kit::NSPopUpMenuWindowLevel);
            w.setCollectionBehavior(
                NSWindowCollectionBehavior::Transient
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            );
            w
        }
        Kind::Bar => {
            let w = panel(mtm, false);
            w.setLevel(objc2_app_kit::NSStatusWindowLevel);
            w.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            );
            w
        }
        Kind::DesktopChild { .. } => {
            let w = panel(mtm, false);
            w.setLevel(CGWindowLevelForKey(CGWindowLevelKey::DesktopWindowLevelKey) as isize + 1);
            w.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            );
            w
        }
    };
    // SAFETY: the surface owns the window and closes it once; AppKit must not release it.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(name));
    if !matches!(kind, Kind::App { .. }) {
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setHasShadow(false);
        if let Some(panel) = window.downcast_ref::<NSPanel>() {
            panel.setFloatingPanel(true);
            panel.setHidesOnDeactivate(false);
            panel.setBecomesKeyOnlyIfNeeded(true);
        }
    }
    window
}

/// Shows `image` (device pixels at `scale` per point) as the view's layer contents.
fn set_contents(view: &View, image: &CGImage, scale: f64) {
    let Some(layer) = view.layer() else { return };
    let contents: &AnyObject = {
        let ptr: *const CGImage = image;
        // SAFETY: a CGImage is an Objective-C object (CF types are toll-free objects).
        unsafe { &*ptr.cast::<AnyObject>() }
    };
    // SAFETY: a CGImage is what `contents` takes.
    unsafe { layer.setContents(Some(contents)) };
    layer.setContentsScale(scale);
}

/// An AppKit window with a retained display list and a CoreGraphics bitmap (or, in GPU mode,
/// a Metal layer).
pub struct Surface {
    id: SurfaceId,
    kind: Kind,
    window: Retained<NSWindow>,
    view: Retained<View>,
    shown: DisplayList,
    canvas: Option<Canvas>,
    origin: (i32, i32),
    size: (u32, u32),
    mode: crate::platform::Render,
    semantics: RefCell<Vec<(HitId, crate::Semantics)>>,
    glass: Cell<Option<crate::Glass>>,
    glass_style: Cell<crate::glass::GlassStyle>,
    glass_open: Cell<bool>,
    /// Native layers by name ([`Surface::native_layer`]): layer-hosting views under the content.
    natives: RefCell<HashMap<String, Native>>,
    #[cfg(feature = "gpu")]
    gpu: Option<Rc<RefCell<super::gpu::Window>>>,
    #[cfg(feature = "gpu")]
    gpu_glass: RefCell<super::gpu::GlassState>,
}

impl Surface {
    /// Creates a new instance.
    pub fn new(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
        Self::with_render(kind, name, role, crate::platform::render())
    }

    /// New deferred accessibility.
    pub fn new_deferred_accessibility(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
        Self::new(kind, name, role)
    }

    /// Chooses CPU or GPU (`gpu` feature: Vello Hybrid on Metal) for this surface alone.
    pub fn with_render(kind: Kind, name: &str, role: accesskit::Role, render: crate::platform::Render) -> Result<Self> {
        let Some(mtm) = MainThreadMarker::new() else { bail!("AppKit windows need the main thread") };
        super::pump::app(mtm, matches!(kind, Kind::App { .. }));
        let id = SurfaceId(NEXT.get());
        NEXT.set(id.0 + 1);
        let window = window(mtm, kind, name);
        let view = View::new(mtm, id, matches!(kind, Kind::App { .. }));
        // AccessKit subclasses the view before anything observes it.
        *view.ivars().a11y.borrow_mut() = Some(super::a11y::A11y::new(&view, id, role, name.into()));
        view.setWantsLayer(true);
        // Files dragged from other apps (see the view's dragging methods).
        // SAFETY: an extern static AppKit defines.
        view.registerForDraggedTypes(&objc2_foundation::NSArray::from_slice(&[unsafe { objc2_app_kit::NSPasteboardTypeFileURL }]));
        view.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::Never);
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
        if let Some(layer) = view.layer() {
            // SAFETY: an extern static QuartzCore defines.
            layer.setContentsGravity(unsafe { kCAGravityTopLeft });
        }
        // A plain container under the view holds system glass beneath it.
        let container = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
        container.setWantsLayer(true);
        window.setContentView(Some(&container));
        view.setFrame(container.bounds());
        container.addSubview(&view);
        window.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*view)));
        window.setInitialFirstResponder(Some(&view));
        window.makeFirstResponder(Some(&view));
        // SAFETY: the view handles the selector; it is removed as an observer on release.
        unsafe {
            NSWorkspace::sharedWorkspace().notificationCenter().addObserver_selector_name_object(
                &view,
                sel!(stillkitSettingChanged:),
                Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
                None,
            );
        }
        VIEWS.with_borrow_mut(|v| v.insert(id, Weak::from_retained(&view)));
        Ok(Self {
            id,
            kind,
            window,
            view,
            shown: DisplayList::default(),
            canvas: None,
            origin: (i32::MIN, i32::MIN),
            size: (0, 0),
            mode: render.resolve(crate::glass::wanted(), crate::glass::cpu_backdrop()),
            semantics: RefCell::default(),
            glass: Cell::new(None),
            glass_style: Cell::new(Default::default()),
            glass_open: Cell::new(false),
            natives: RefCell::default(),
            #[cfg(feature = "gpu")]
            gpu: None,
            #[cfg(feature = "gpu")]
            gpu_glass: RefCell::default(),
        })
    }

    /// Render.
    pub fn render(&self) -> crate::platform::Render {
        self.mode
    }

    fn is_app(&self) -> bool {
        matches!(self.kind, Kind::App { .. })
    }

    /// Draws `list` where it differs from the one on screen, at `origin` (screen px; app
    /// windows keep their own frame) and `size` device pixels. Whether anything was drawn.
    pub fn present(&mut self, ui: &mut Ui, origin: (i32, i32), size: (u32, u32), list: DisplayList) -> Result<bool> {
        let resized = self.size != size;
        if !self.is_app() && (resized || self.origin != origin) {
            self.frame(origin, size);
        }
        self.origin = origin;
        self.set_hits(&list);
        #[cfg(feature = "gpu")]
        let glass_changed = self.gpu.as_ref().is_some_and(|g| g.borrow().glass_revision() != self.gpu_glass.borrow().revision);
        #[cfg(not(feature = "gpu"))]
        let glass_changed = false;
        let fresh = resized || glass_changed || (self.canvas.is_none() && self.mode == crate::platform::Render::Cpu);
        let whole = crate::Rect::new(0., 0., size.0 as f32, size.1 as f32);
        // Disjoint damage repaints each region alone (a menu and a counter far apart need not
        // repaint the window between them); the GPU path takes their union.
        let damage = if fresh { vec![whole] } else { list.damage_regions(&self.shown) };
        let regions: Vec<_> = damage.iter().filter_map(|d| d.pixels(size.0, size.1)).collect();
        let Some(area) = damage.into_iter().reduce(crate::Rect::union).and_then(|d| d.pixels(size.0, size.1)) else {
            self.shown = list;
            return Ok(false);
        };
        #[cfg(feature = "gpu")]
        if self.mode == crate::platform::Render::Gpu {
            let scale = self.view.scale();
            let gpu = match &self.gpu {
                Some(gpu) => gpu.clone(),
                None => {
                    // SAFETY: the surface owns the view and drops the GPU window first.
                    let gpu = Rc::new(RefCell::new(unsafe { super::gpu::Window::new(&self.view, size) }?));
                    super::gpu::register(&gpu);
                    self.gpu = Some(gpu.clone());
                    gpu
                }
            };
            gpu.borrow_mut().draw(ui, &list, size, area, &self.gpu_glass.borrow(), scale as f32)?;
            self.size = size;
            self.shown = list;
            return Ok(true);
        }
        if resized || self.canvas.is_none() {
            self.canvas = Some(Canvas::new(ui, size.0, size.1)?);
        }
        let canvas = self.canvas.as_ref().context("no canvas")?;
        for region in &regions {
            canvas.draw(&mut ui.text, &list, *region)?;
        }
        if let Some(image) = canvas.cg_image() {
            set_contents(&self.view, &image, self.view.scale());
        }
        self.size = size;
        self.shown = list;
        Ok(true)
    }

    /// Puts a non-app window's frame at `origin` (screen px) with `size` device pixels.
    fn frame(&self, origin: (i32, i32), size: (u32, u32)) {
        let s = self.window.backingScaleFactor();
        let frame = super::screen::frame_at(origin, (size.0 as f64 / s, size.1 as f64 / s));
        self.window.setFrame_display(frame, false);
        self.place_glass();
    }

    fn set_hits(&self, list: &DisplayList) {
        let mut hits = self.view.ivars().hits.borrow_mut();
        hits.clear();
        hits.extend(list.hits.iter().map(|h| h.rect));
        drop(hits);
        let events = self.view.ivars().a11y.try_borrow_mut().ok().and_then(|mut a| {
            a.as_mut()?.update(&list.hits, list.focus, &self.semantics.borrow())
        });
        if let Some(events) = events {
            events.raise();
        }
    }

    /// The retained frame, for screenshots (a GPU frame waits for the GPU).
    pub fn image(&self) -> Result<crate::Image> {
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &self.gpu {
            return gpu.borrow().image();
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

    /// Animation frames.
    pub fn animation_frames(&self, duration: Duration) -> AnimationFrames {
        let active = Rc::new(Cell::new(true));
        super::pump::frames(self.id, active.clone(), Instant::now() + duration);
        AnimationFrames(active)
    }

    /// The window's opacity, applied by the window server (no redraw).
    pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
        self.window.setAlphaValue(alpha as f64 / 255.0);
        Ok(())
    }

    /// Move to.
    pub fn move_to(&mut self, origin: (i32, i32)) -> Result<()> {
        if origin != self.origin {
            self.origin = origin;
            if !self.is_app() {
                self.frame(origin, self.size);
            }
        }
        Ok(())
    }

    /// Optional metadata for the next presented frame.
    pub fn set_accessibility_semantics(&self, semantics: Vec<(HitId, crate::Semantics)>) {
        *self.semantics.borrow_mut() = semantics;
    }

    /// Consumes VoiceOver's rich request when handling its `Event::Invoke`.
    pub fn take_accessibility_action(&self, hit: HitId) -> Option<crate::AccessibilityAction> {
        self.view.ivars().a11y.borrow().as_ref()?.take_action(hit)
    }

    /// Puts glass under `glass.body`: a GPU surface draws it in-process; a CPU surface asks the
    /// thread's [`Backdrop`](crate::glass::Backdrop) (an app's, or the system's frost).
    pub fn set_glass(&self, glass: Option<crate::Glass>) {
        self.set_glass_group(glass.as_slice(), 16.0);
    }

    /// Sets glass style.
    pub fn set_glass_style(&self, style: crate::glass::GlassStyle) {
        if self.glass_style.replace(style) == style {
            return;
        }
        #[cfg(feature = "gpu")]
        if self.mode == crate::platform::Render::Gpu {
            let mut g = self.gpu_glass.borrow_mut();
            g.style = style;
            g.revision += 1;
            return;
        }
        if self.glass_open.get()
            && let Some(backdrop) = crate::glass::backdrop()
        {
            backdrop.style(self.id.0 as usize, &style);
        }
    }

    /// Supplies premultiplied BGRA pixels for in-process glass to refract (no desktop capture
    /// without Screen Recording permission); without them a generated pattern stands in.
    #[cfg(feature = "gpu")]
    pub fn set_glass_backdrop(&self, image: crate::Image) -> Result<()> {
        anyhow::ensure!(
            image.width > 0 && image.height > 0 && image.pixels.len() == image.width as usize * image.height as usize * 4,
            "invalid glass backdrop"
        );
        let mut g = self.gpu_glass.borrow_mut();
        g.backdrop = Some(image);
        g.backdrop_revision += 1;
        g.revision += 1;
        Ok(())
    }

    /// Whether glass can show under this surface: in-process on a GPU surface, else through
    /// the thread's backdrop.
    pub fn glass_available(&self) -> bool {
        self.mode == crate::platform::Render::Gpu || crate::glass::available()
    }

    /// Several glass bodies: on a GPU surface one liquid container (they fuse within
    /// `spacing` px); a CPU surface's backdrop shows the first.
    pub fn set_glass_group(&self, glasses: &[crate::Glass], spacing: f32) {
        #[cfg(feature = "gpu")]
        if self.mode == crate::platform::Render::Gpu {
            let mut g = self.gpu_glass.borrow_mut();
            if g.glasses != glasses || g.spacing != spacing {
                g.glasses = glasses.to_vec();
                g.spacing = spacing;
                g.revision += 1;
            }
            return;
        }
        let _ = spacing;
        let new = glasses.first().copied();
        let old = self.glass.replace(new);
        if !self.glass_open.get() {
            if new.is_some() && self.is_visible() {
                self.open_glass();
            }
            return;
        }
        let (Some(new), Some(backdrop)) = (new, crate::glass::backdrop()) else {
            self.close_glass();
            return;
        };
        if old.is_none_or(|old| (old.body, old.radius) != (new.body, new.radius)) {
            backdrop.place(self.id.0 as usize, new.shape(self.screen_origin()), new.radius);
        }
        if old.is_none_or(|old| old.material != new.material) {
            backdrop.material(self.id.0 as usize, &new.material);
        }
    }

    /// The content's top-left on screen (app windows place themselves).
    fn screen_origin(&self) -> (i32, i32) {
        if self.is_app() || self.origin == (i32::MIN, i32::MIN) {
            let frame = self.window.contentRectForFrameRect(self.window.frame());
            super::screen::to_px(NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height))
        } else {
            self.origin
        }
    }

    fn open_glass(&self) {
        let (Some(glass), Some(backdrop)) = (self.glass.get(), crate::glass::backdrop()) else { return };
        let opened = backdrop.open_styled(
            self.id.0 as usize,
            glass.shape(self.screen_origin()),
            glass.radius,
            &glass.material,
            &self.glass_style.get(),
        );
        self.glass_open.set(opened);
    }

    fn place_glass(&self) {
        if self.glass_open.get()
            && let (Some(glass), Some(backdrop)) = (self.glass.get(), crate::glass::backdrop())
        {
            backdrop.place(self.id.0 as usize, glass.shape(self.screen_origin()), glass.radius);
        }
    }

    fn close_glass(&self) {
        if self.glass_open.replace(false)
            && let Some(backdrop) = crate::glass::backdrop()
        {
            backdrop.close(self.id.0 as usize);
        }
    }

    /// Orders the window front without activating the app (`App` windows: [`show_active`]).
    pub fn show(&self) {
        self.window.orderFrontRegardless();
        if self.glass.get().is_some() && !self.glass_open.get() {
            self.open_glass();
        }
    }

    /// Hide.
    pub fn hide(&self) {
        self.close_glass();
        self.window.orderOut(None);
    }

    /// Whether visible.
    pub fn is_visible(&self) -> bool {
        self.window.isVisible()
    }

    /// Lets clicks through the whole window (`true`), or only through alpha 0 (`false`).
    /// AppKit hit-tests a transparent window by its alpha only while `ignoresMouseEvents` was
    /// never set, so `false` after `true` makes the whole window take clicks.
    pub fn set_click_through(&self, through: bool) {
        if through || self.window.ignoresMouseEvents() {
            self.window.setIgnoresMouseEvents(through);
        }
    }

    /// Frees the bitmap (a hidden surface keeps none); the next `present` draws everything.
    pub fn release(&mut self) {
        self.canvas = None;
        self.size = (0, 0);
        self.shown = DisplayList::default();
    }

    /// Discard events.
    pub fn discard_events(&self) {
        super::pump::discard(self.id);
    }

    /// Makes an activating popup (or an app window) the key window and activates the app.
    pub fn take_focus(&self) -> &'static str {
        if matches!(self.kind, Kind::Popup { activate: false } | Kind::Bar | Kind::DesktopChild { .. }) {
            return "suppressed";
        }
        let Some(mtm) = MainThreadMarker::new() else { return "refused" };
        objc2_app_kit::NSApplication::sharedApplication(mtm).activate();
        self.window.makeKeyAndOrderFront(None);
        if self.window.isKeyWindow() { "plain" } else { "refused" }
    }

    // App-window services (`Kind::App`).

    /// Device pixels per point (2 on Retina screens).
    pub fn scale(&self) -> f32 {
        self.view.scale() as f32
    }

    /// Where the window is, in screen pixels: `(x, y, width, height)`.
    pub fn place(&self, (x, y, w, h): (i32, i32, i32, i32)) {
        let s = self.window.backingScaleFactor();
        self.window.setFrame_display(super::screen::frame_at((x, y), (w.max(1) as f64 / s, h.max(1) as f64 / s)), false);
    }

    /// Where the window is now, in screen pixels as [`place`](Self::place) takes it
    /// (`x, y, width, height`): after the person moved or resized it.
    pub fn placed(&self) -> Option<(i32, i32, i32, i32)> {
        let (l, t, r, b) = super::screen::edges(self.window.frame());
        Some((l, t, r - l, b - t))
    }

    /// The content's size in pixels.
    pub fn client_size(&self) -> (u32, u32) {
        self.view.client_size()
    }

    /// Shows an app window, makes it key and activates the app (`maximized`: zoomed).
    pub fn show_active(&self, maximized: bool) {
        if let Some(mtm) = MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(mtm).activate();
        }
        self.window.makeKeyAndOrderFront(None);
        if maximized && !self.window.isZoomed() {
            self.window.zoom(None);
        }
    }

    /// Whether maximized.
    pub fn is_maximized(&self) -> bool {
        self.window.isZoomed()
    }

    /// Minimize.
    pub fn minimize(&self) {
        self.window.miniaturize(None);
    }

    /// Toggle maximize.
    pub fn toggle_maximize(&self) {
        self.window.zoom(None);
    }

    /// Whether the window fills its screen in its own full-screen space.
    pub fn is_fullscreen(&self) -> bool {
        self.window
            .styleMask()
            .contains(NSWindowStyleMask::FullScreen)
    }

    /// Enter or leave full screen: the system's own space for the window, as the green
    /// button gives it (an app window opts in first).
    pub fn toggle_fullscreen(&self) {
        let behavior = self.window.collectionBehavior();
        if !behavior.contains(NSWindowCollectionBehavior::FullScreenPrimary) {
            self.window
                .setCollectionBehavior(behavior | NSWindowCollectionBehavior::FullScreenPrimary);
        }
        self.window.toggleFullScreen(None);
    }

    /// The title area's parts, in client pixels (see [`Chrome`]). Later entries win.
    pub fn set_chrome(&self, parts: &[(crate::Rect, Chrome)]) {
        let mut chrome = self.view.ivars().chrome.borrow_mut();
        if chrome.as_slice() != parts {
            *chrome = parts.to_vec();
        }
    }

    /// What shows where the window has grown and the app hasn't drawn yet.
    pub fn set_background(&self, color: crate::Rgba) {
        let c = NSColor::colorWithSRGBRed_green_blue_alpha(color.r as f64, color.g as f64, color.b as f64, color.a as f64);
        self.window.setBackgroundColor(Some(&c));
        if self.is_app() {
            self.window.setOpaque(color.a >= 1.0);
        }
    }

    /// Sets cursor.
    pub fn set_cursor(&self, cursor: Cursor) {
        self.view.set_cursor(cursor);
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.close_glass();
        #[cfg(feature = "gpu")]
        drop(self.gpu.take());
        // SAFETY: the view was added as an observer when made.
        unsafe { NSWorkspace::sharedWorkspace().notificationCenter().removeObserver(&self.view) };
        // The adapter retains the view: drop it to break the cycle.
        drop(self.view.ivars().a11y.take());
        self.window.setDelegate(None);
        self.window.close();
        VIEWS.with_borrow_mut(|v| v.remove(&self.id));
        super::pump::discard(self.id);
    }
}

impl crate::platform::Window for Surface {
    type Bitmap = crate::Image;
    fn new(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
        Self::new(kind, name, role)
    }
    /// Shows a whole bitmap made elsewhere (the dirty rect is redrawn by the window server
    /// from the new contents) and moves the window to `origin`.
    fn present_bitmap(&mut self, origin: (i32, i32), bitmap: Option<(&crate::Image, crate::PixelRect, bool)>) -> Result<()> {
        if let Some((image, _, _)) = bitmap {
            let size = (image.width, image.height);
            if !self.is_app() && (size != self.size || origin != self.origin) {
                self.frame(origin, size);
            }
            self.size = size;
            let cg = super::draw::cg_image(image).context("CGImage")?;
            set_contents(&self.view, &cg, self.view.scale());
        } else if !self.is_app() && origin != self.origin {
            self.frame(origin, self.size);
        }
        self.origin = origin;
        Ok(())
    }
    fn next_event() -> Option<(SurfaceId, Event)> {
        next_event()
    }
}

/// A native layer: a layer-hosting view with a `CAMetalLayer`, below the content view.
struct Native {
    view: Retained<NSView>,
    layer: Retained<objc2_quartz_core::CALayer>,
    /// The device-px rect it was last placed at.
    rect: Option<crate::Rect>,
}

impl Surface {
    /// The native layer `name`, made on first use: a `CAMetalLayer` in a layer-hosting view
    /// beneath the surface's content, where the host draws with its own GPU device. It shows
    /// only where the content is transparent ([`crate::Op::Hole`], drawn by
    /// [`crate::el::native`]) and once placed ([`place_native_layer`](Self::place_native_layer)).
    /// Making one loads no GPU driver: the layer has no device until the host gives it one.
    pub fn native_layer(&self, name: &str) -> Result<crate::NativeHandle> {
        if let Some(n) = self.natives.borrow().get(name) {
            return Ok(crate::NativeHandle::Metal(Retained::as_ptr(&n.layer).cast_mut().cast()));
        }
        let mtm = MainThreadMarker::new().context("AppKit views need the main thread")?;
        // The window's content view is the container the surface's view hangs in.
        let container = self.window.contentView().context("the surface's window has no container")?;
        let class = objc2::runtime::AnyClass::get(c"CAMetalLayer").context("QuartzCore has no CAMetalLayer")?;
        // SAFETY: `+layer` is CALayer's autoreleased constructor; CAMetalLayer is a CALayer.
        let layer: Retained<objc2_quartz_core::CALayer> = unsafe { msg_send![class, layer] };
        layer.setContentsScale(self.window.backingScaleFactor());
        let view = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
        // Layer-hosting: the layer is set before the view wants one.
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view.setHidden(true);
        container.addSubview_positioned_relativeTo(&view, objc2_app_kit::NSWindowOrderingMode::Below, Some(&self.view));
        let handle = crate::NativeHandle::Metal(Retained::as_ptr(&layer).cast_mut().cast());
        self.natives.borrow_mut().insert(name.to_owned(), Native { view, layer, rect: None });
        Ok(handle)
    }

    /// Shows native layer `name` at `rect` (device px from the client's top-left), or hides it
    /// (`None`). Unchanged rects cost nothing; unknown names are ignored.
    pub fn place_native_layer(&self, name: &str, rect: Option<crate::Rect>) {
        let mut natives = self.natives.borrow_mut();
        let Some(n) = natives.get_mut(name) else { return };
        if n.rect == rect {
            return;
        }
        n.rect = rect;
        let Some(r) = rect else {
            n.view.setHidden(true);
            return;
        };
        let s = self.window.backingScaleFactor();
        let height = self.window.contentView().map_or(0.0, |c| c.bounds().size.height);
        // The container isn't flipped: AppKit's origin is its bottom-left.
        let frame = NSRect::new(
            NSPoint::new(f64::from(r.left) / s, height - f64::from(r.bottom) / s),
            NSSize::new(f64::from(r.width()) / s, f64::from(r.height()) / s),
        );
        n.view.setFrame(frame);
        n.layer.setContentsScale(s);
        n.view.setHidden(false);
    }

    /// Removes native layer `name`; its handle is invalid afterwards.
    pub fn release_native_layer(&self, name: &str) {
        if let Some(n) = self.natives.borrow_mut().remove(name) {
            n.view.removeFromSuperview();
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

/// Keeps dropped paths for [`take_dropped_files`].
#[allow(dead_code)]
pub(super) fn set_dropped_files(paths: Vec<std::path::PathBuf>) {
    DROPPED.with_borrow_mut(|d| *d = paths);
}

impl Surface {
    /// How tall the title area is where the traffic lights sit: [`Caption::Compact`] gives the
    /// window an empty compact toolbar, which makes the title area 40 points and centres the
    /// lights in it, for an app's own taller title bar. App windows only.
    ///
    /// [`Caption::Compact`]: crate::Caption::Compact
    pub fn set_caption(&self, caption: crate::Caption) {
        if !self.is_app() {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else { return };
        match caption {
            crate::Caption::Standard => self.window.setToolbar(None),
            crate::Caption::Compact => {
                if self.window.toolbar().is_none() {
                    let toolbar = objc2_app_kit::NSToolbar::new(mtm);
                    self.window.setToolbar(Some(&toolbar));
                }
                self.window.setToolbarStyle(objc2_app_kit::NSWindowToolbarStyle::UnifiedCompact);
            }
        }
    }

    /// Where the system draws the window controls (the traffic lights, close to zoom), in
    /// device px from the client's top-left, so an app's title bar leaves them room. `None`
    /// in full screen, where they are hidden, and for non-app surfaces.
    pub fn system_controls(&self) -> Option<crate::Rect> {
        if !self.is_app() || self.window.styleMask().contains(NSWindowStyleMask::FullScreen) {
            return None;
        }
        let close = self.window.standardWindowButton(objc2_app_kit::NSWindowButton::CloseButton)?;
        let zoom = self.window.standardWindowButton(objc2_app_kit::NSWindowButton::ZoomButton)?;
        let height = self.window.contentView()?.bounds().size.height;
        let s = self.window.backingScaleFactor();
        let frame = |b: &objc2_app_kit::NSButton| b.convertRect_toView(b.bounds(), None);
        let (c, z) = (frame(&close), frame(&zoom));
        let px = |v: f64| (v * s) as f32;
        Some(crate::Rect {
            left: px(c.origin.x),
            top: px(height - (c.origin.y + c.size.height)),
            right: px(z.origin.x + z.size.width),
            bottom: px(height - c.origin.y),
        })
    }
}
