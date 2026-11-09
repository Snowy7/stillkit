//! The NSView every surface draws in, and its window's delegate: AppKit's callbacks become
//! queued [`Event`]s. Its layer's contents are the surface's bitmap (set by `Surface`); this
//! view never draws. It is the NSTextInputClient for typing and input methods (the marked text
//! is `el`'s preedit, the candidate window opens at the field's caret) and the view AccessKit
//! subclasses for VoiceOver.
use super::app_window::{Chrome, Cursor};
use super::surface::{Button, Event, Pointer, SurfaceId};
use super::{ime, keys, pump, screen};
use crate::{Point, Rect};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSCursor, NSEvent, NSEventModifierFlags, NSResponder, NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions,
    NSView, NSWindowDelegate,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound, NSNotification, NSObjectProtocol,
    NSPoint, NSRange, NSRangePointer, NSRect, NSSize, NSString, NSUInteger,
};
use std::cell::{Cell, RefCell};

/// What a view knows of its surface.
pub(super) struct Ivars {
    pub id: SurfaceId,
    /// An app window ([`Kind::App`](super::Kind::App)): chrome, first click only activates.
    pub app: bool,
    pub chrome: RefCell<Vec<(Rect, Chrome)>>,
    /// The shown hit rectangles: the caption never takes a click meant for a control.
    pub hits: RefCell<Vec<Rect>>,
    tracking: RefCell<Option<Retained<NSTrackingArea>>>,
    /// The focused field's caret (device px) while one types in this view.
    pub caret: Cell<Option<Rect>>,
    /// The input method's composition, while there is one.
    pub marked: RefCell<String>,
    pub cursor: Cell<Cursor>,
    pub a11y: RefCell<Option<super::a11y::A11y>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements; `View` doesn't implement Drop.
    #[unsafe(super(NSView, NSResponder, objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "StillkitView"]
    #[ivars = Ivars]
    pub(super) struct View;

    impl View {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        /// Popups, bars and desktop surfaces take the click that activates them; an app window
        /// takes its first click as activation only, as macOS apps do.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            !self.ivars().app
        }

        #[unsafe(method(wantsUpdateLayer))]
        fn wants_update_layer(&self) -> bool {
            true
        }

        /// The surface sets the layer's contents itself.
        #[unsafe(method(updateLayer))]
        fn update_layer(&self) {}

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if !self.chrome(event) {
                self.press(event, Button::Left);
            }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.press(event, Button::Right);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            if event.buttonNumber() == 2 {
                self.press(event, Button::Middle);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.send(Event::Release(self.pointer(event), Button::Left));
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.send(Event::Release(self.pointer(event), Button::Right));
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            if event.buttonNumber() == 2 {
                self.send(Event::Release(self.pointer(event), Button::Middle));
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.cursor_shape().set();
            self.send(Event::Move(self.pointer(event)));
        }

        // A drag keeps the pointer: it goes to the view it started in, inside or not.
        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.send(Event::Move(self.pointer(event)));
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.send(Event::Move(self.pointer(event)));
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.send(Event::Move(self.pointer(event)));
        }

        /// Files dragged from Finder or another app: hover, leave and drop, so views can show
        /// a drop target and take the paths.
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, info: &AnyObject) -> usize {
            self.files_over(info)
        }

        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(&self, info: &AnyObject) -> usize {
            self.files_over(info)
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, _info: Option<&AnyObject>) {
            self.send(Event::FilesLeave);
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, info: &AnyObject) -> objc2::runtime::Bool {
            let paths = dragged_files(info);
            if paths.is_empty() {
                return objc2::runtime::Bool::NO;
            }
            super::surface::set_dropped_files(paths);
            self.send(Event::FilesDropped(self.drag_pointer(info)));
            objc2::runtime::Bool::YES
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.send(Event::Leave);
        }

        /// Wheel notches are 120 a line; a trackpad's precise points scroll that many points
        /// (120 per [`crate::scroll::NOTCH`]). Positive scrolls up, as on Windows.
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let dy = event.scrollingDeltaY();
            let delta = if event.hasPreciseScrollingDeltas() {
                dy * 120.0 / crate::scroll::NOTCH as f64
            } else {
                dy * 120.0
            };
            let delta = delta.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            if delta != 0 {
                self.send(Event::Wheel(self.pointer(event), delta));
            }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let id = self.ivars().id;
            let composing = !self.ivars().marked.borrow().is_empty();
            // While the input method composes, its keys (arrows, Return, Escape) are its own.
            if !composing {
                let unmodified = event.charactersIgnoringModifiers().and_then(|s| s.to_string().chars().next());
                if let Some(vk) = keys::vk(event.keyCode(), unmodified) {
                    self.send(Event::Key(vk));
                    if let Some(e) = super::text_field::key(id, Some(vk), "") {
                        self.send(e);
                    }
                }
            }
            // Shortcuts type nothing; everything else goes through the input context (dead
            // keys, input methods), which calls back with `insertText:` or `setMarkedText:`.
            let shortcut = event.modifierFlags().intersects(NSEventModifierFlags::Command | NSEventModifierFlags::Control);
            if composing || !shortcut {
                self.interpretKeyEvents(&NSArray::from_slice(&[event]));
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            let unmodified = event.charactersIgnoringModifiers().and_then(|s| s.to_string().chars().next());
            if let Some(vk) = keys::vk(event.keyCode(), unmodified) {
                self.send(Event::KeyUp(vk));
            }
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            if let Some(old) = self.ivars().tracking.take() {
                self.removeTrackingArea(&old);
            }
            let options = NSTrackingAreaOptions::MouseEnteredAndExited
                | NSTrackingAreaOptions::MouseMoved
                | NSTrackingAreaOptions::ActiveAlways
                | NSTrackingAreaOptions::InVisibleRect;
            // SAFETY: the view owns the area and removes it before replacing it.
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    NSRect::ZERO,
                    options,
                    Some(self),
                    None,
                )
            };
            self.addTrackingArea(&area);
            *self.ivars().tracking.borrow_mut() = Some(area);
            // SAFETY: the superclass's own implementation.
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn appearance_changed(&self) {
            self.send(Event::SettingChanged);
        }

        /// Reduce motion (and other display options) changed: NSWorkspace's notification.
        #[unsafe(method(stillkitSettingChanged:))]
        fn setting_changed(&self, _note: &NSNotification) {
            self.send(Event::SettingChanged);
        }
    }

    unsafe impl NSObjectProtocol for View {}

    unsafe impl NSWindowDelegate for View {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &objc2_app_kit::NSWindow) -> bool {
            self.send(Event::Close);
            false
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _note: &NSNotification) {
            if self.ivars().app {
                let (w, h) = self.client_size();
                self.send(Event::Resized(w, h));
                if self.inLiveResize() {
                    super::app_window::run_modal_hook();
                }
            }
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, _note: &NSNotification) {
            self.focus_state(true);
            self.send(Event::Activated);
        }

        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _note: &NSNotification) {
            self.focus_state(false);
            self.send(Event::Deactivated);
        }

        #[unsafe(method(windowDidChangeBackingProperties:))]
        fn window_did_change_backing(&self, _note: &NSNotification) {
            self.send(Event::DpiChanged((96.0 * self.scale()).round() as u32));
        }
    }

    unsafe impl NSTextInputClient for View {
        #[unsafe(method(insertText:replacementRange:))]
        unsafe fn insert_text(&self, string: &AnyObject, _replacement: NSRange) {
            let text = text_of(string);
            if !self.ivars().marked.take().is_empty() {
                ime::set_preedit(String::new(), 0);
                self.send(Event::Preedit);
            }
            for c in text.chars().filter(|c| !c.is_control()) {
                self.send(Event::Char(c));
            }
            if let Some(e) = super::text_field::key(self.ivars().id, None, &text) {
                self.send(e);
            }
        }

        #[unsafe(method(doCommandBySelector:))]
        unsafe fn do_command(&self, _selector: Sel) {
            // The key went out as `Event::Key` already; no beep.
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        unsafe fn set_marked_text(&self, string: &AnyObject, selected: NSRange, _replacement: NSRange) {
            let text = text_of(string);
            let cursor = super::text::byte_at(&text, selected.location);
            *self.ivars().marked.borrow_mut() = text.clone();
            ime::set_preedit(text, cursor);
            self.send(Event::Preedit);
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            if !self.ivars().marked.take().is_empty() {
                ime::set_preedit(String::new(), 0);
                self.send(Event::Preedit);
            }
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            NSRange::new(NSNotFound as NSUInteger, 0)
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            let marked = self.ivars().marked.borrow();
            if marked.is_empty() {
                NSRange::new(NSNotFound as NSUInteger, 0)
            } else {
                NSRange::new(0, marked.encode_utf16().count())
            }
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            !self.ivars().marked.borrow().is_empty()
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        unsafe fn attributed_substring(&self, _range: NSRange, _actual: NSRangePointer) -> Option<Retained<NSAttributedString>> {
            None
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new()
        }

        /// Where the candidate window opens: under the focused field's caret (screen points).
        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        unsafe fn first_rect(&self, _range: NSRange, _actual: NSRangePointer) -> NSRect {
            let s = self.scale();
            let local = match self.ivars().caret.get() {
                Some(c) => NSRect::new(
                    NSPoint::new(c.left as f64 / s, c.top as f64 / s),
                    NSSize::new((c.width() as f64 / s).max(1.0), c.height() as f64 / s),
                ),
                None => self.bounds(),
            };
            let in_window = self.convertRect_toView(local, None);
            self.window().map_or(in_window, |w| w.convertRectToScreen(in_window))
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index(&self, _point: NSPoint) -> NSUInteger {
            NSNotFound as NSUInteger
        }
    }
);

/// A string AppKit passes as `NSString` or `NSAttributedString`.
fn text_of(string: &AnyObject) -> String {
    if let Some(s) = string.downcast_ref::<NSAttributedString>() {
        s.string().to_string()
    } else if let Some(s) = string.downcast_ref::<NSString>() {
        s.to_string()
    } else {
        String::new()
    }
}

impl View {
    pub(super) fn new(mtm: MainThreadMarker, id: SurfaceId, app: bool) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars {
            id,
            app,
            chrome: RefCell::default(),
            hits: RefCell::default(),
            tracking: RefCell::default(),
            caret: Cell::new(None),
            marked: RefCell::default(),
            cursor: Cell::new(Cursor::Arrow),
            a11y: RefCell::new(None),
        });
        // SAFETY: NSView's designated initializer.
        unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] }
    }

    fn send(&self, event: Event) {
        pump::push(self.ivars().id, event);
    }

    /// The window's backing scale: device pixels per point.
    pub(super) fn scale(&self) -> f64 {
        self.window().map_or_else(|| screen::primary().scale, |w| w.backingScaleFactor())
    }

    /// The view's size in device pixels.
    pub(super) fn client_size(&self) -> (u32, u32) {
        let (b, s) = (self.bounds().size, self.scale());
        ((b.width * s).round() as u32, (b.height * s).round() as u32)
    }

    fn pointer(&self, event: &NSEvent) -> Pointer {
        let local = self.convertPoint_fromView(event.locationInWindow(), None);
        let s = self.scale();
        Pointer { at: Point::new((local.x * s) as f32, (local.y * s) as f32), screen: screen::to_px(NSEvent::mouseLocation()) }
    }

    fn press(&self, event: &NSEvent, button: Button) {
        let p = self.pointer(event);
        // The second click of a double click comes as one, as Windows sends it.
        self.send(if event.clickCount() == 2 { Event::DoubleClick(p, button) } else { Event::Press(p, button) });
    }

    /// An app window's own title area: drags, zooms, miniaturizes or closes. Whether the
    /// press was the chrome's.
    fn chrome(&self, event: &NSEvent) -> bool {
        let Some(window) = self.window().filter(|_| self.ivars().app) else { return false };
        let at = self.pointer(event).at;
        let part = self.ivars().chrome.borrow().iter().rev().find(|(r, _)| r.contains(at)).map(|(_, c)| *c);
        match part {
            Some(Chrome::Caption) if !self.ivars().hits.borrow().iter().any(|h| h.contains(at)) => {
                if event.clickCount() == 2 {
                    window.performZoom(None);
                } else {
                    window.performWindowDragWithEvent(event);
                }
                true
            }
            Some(Chrome::Minimize) => {
                window.miniaturize(None);
                true
            }
            Some(Chrome::Maximize) => {
                window.zoom(None);
                true
            }
            Some(Chrome::Close) => {
                self.send(Event::Close);
                true
            }
            _ => false,
        }
    }

    fn cursor_shape(&self) -> Retained<NSCursor> {
        match self.ivars().cursor.get() {
            Cursor::Arrow | Cursor::Resize => NSCursor::arrowCursor(),
            Cursor::Text => NSCursor::IBeamCursor(),
            Cursor::Hand => NSCursor::pointingHandCursor(),
            Cursor::Move => NSCursor::openHandCursor(),
            #[allow(deprecated)]
            Cursor::ResizeHorizontal => NSCursor::resizeLeftRightCursor(),
            #[allow(deprecated)]
            Cursor::ResizeVertical => NSCursor::resizeUpDownCursor(),
            Cursor::Grab => NSCursor::openHandCursor(),
            Cursor::Grabbing => NSCursor::closedHandCursor(),
            Cursor::Crosshair => NSCursor::crosshairCursor(),
            Cursor::NotAllowed => NSCursor::operationNotAllowedCursor(),
            Cursor::Copy => NSCursor::dragCopyCursor(),
        }
    }

    pub(super) fn set_cursor(&self, cursor: Cursor) {
        self.ivars().cursor.set(cursor);
        self.cursor_shape().set();
    }

    fn focus_state(&self, focused: bool) {
        let events = self.ivars().a11y.try_borrow_mut().ok().and_then(|mut a| a.as_mut()?.focus_changed(focused));
        if let Some(events) = events {
            events.raise();
        }
    }
}

impl View {
    /// Files over the view: where, as a pointer; the copy operation when they are files.
    fn files_over(&self, info: &AnyObject) -> usize {
        if dragged_files(info).is_empty() {
            return 0; // NSDragOperationNone
        }
        self.send(Event::FilesHover(self.drag_pointer(info)));
        1 // NSDragOperationCopy
    }

    /// A drag's place as a pointer in the view's device pixels.
    fn drag_pointer(&self, info: &AnyObject) -> Pointer {
        // SAFETY: NSDraggingInfo's `draggingLocation`, in window coordinates.
        let at: NSPoint = unsafe { msg_send![info, draggingLocation] };
        let local = self.convertPoint_fromView(at, None);
        let s = self.scale();
        Pointer { at: Point::new((local.x * s) as f32, (local.y * s) as f32), screen: screen::to_px(NSEvent::mouseLocation()) }
    }
}

/// The file paths a drag carries (file URLs on its pasteboard).
fn dragged_files(info: &AnyObject) -> Vec<std::path::PathBuf> {
    use objc2_app_kit::NSPasteboard;
    // SAFETY: NSDraggingInfo's `draggingPasteboard`, an NSPasteboard.
    let board: Option<Retained<NSPasteboard>> = unsafe { msg_send![info, draggingPasteboard] };
    let Some(board) = board else { return Vec::new() };
    let Some(items) = board.pasteboardItems() else { return Vec::new() };
    // SAFETY: an extern static AppKit defines.
    let file_url = unsafe { objc2_app_kit::NSPasteboardTypeFileURL };
    items
        .iter()
        .filter_map(|item| item.stringForType(file_url))
        .filter_map(|url| {
            let url = objc2_foundation::NSURL::URLWithString(&url)?;
            url.path().map(|p| std::path::PathBuf::from(p.to_string()))
        })
        .collect()
}
