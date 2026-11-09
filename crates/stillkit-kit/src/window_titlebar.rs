//! Custom title bands reuse the platform's native caption hit testing.
use crate::*;
use stillkit::el::Runtime;
use stillkit::{Chrome, Rect, Surface};

/// Keyboard/assistive window actions; the host applies them to its surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowAction {
    /// Minimize the window.
    Minimize,
    /// Toggle maximized/restored size.
    Maximize,
    /// Ask to close, preserving the app's unsaved-document policy.
    Close,
}
/// Titlebar with leading, centre and trailing slots. Call `sync_chrome` after layout.
pub struct WindowTitlebar<S> {
    key: Arc<str>,
    leading: Vec<El<S>>,
    centre: Option<El<S>>,
    trailing: Vec<El<S>>,
    inset: f32,
    native_buttons: bool,
    action: Change<S, WindowAction>,
}
impl<S: 'static> WindowTitlebar<S> {
    /// The action callback handles keyboard and assistive activation of caption buttons.
    pub fn new(key: impl Into<Arc<str>>, action: impl Fn(&mut S, WindowAction) + 'static) -> Self {
        Self {
            key: key.into(),
            leading: Vec::new(),
            centre: None,
            trailing: Vec::new(),
            inset: 0.,
            native_buttons: cfg!(windows),
            action: Arc::new(action),
        }
    }
    /// Reserve `Surface::system_controls().right / surface.scale()` on macOS.
    pub fn system_inset(mut self, inset: f32) -> Self {
        self.inset = inset.max(0.);
        self
    }
    /// Draw caption buttons (default Windows only).
    pub fn buttons(mut self, yes: bool) -> Self {
        self.native_buttons = yes;
        self
    }
    /// Add a menu or app icon before the flexible centre slot.
    pub fn leading(mut self, e: El<S>) -> Self {
        self.leading.push(e);
        self
    }
    /// Place tabs or a title in the flexible centre slot.
    pub fn tabs(mut self, e: El<S>) -> Self {
        self.centre = Some(e);
        self
    }
    /// Add search or menus before the caption buttons.
    pub fn trailing(mut self, e: El<S>) -> Self {
        self.trailing.push(e);
        self
    }
    /// Build the 40-pixel caption band, with interactive children excluded by native hit testing.
    pub fn build(self) -> El<S> {
        let mut r = row()
            .key(self.key.clone())
            .h(40.)
            .w_full()
            .gap(token::ITEM)
            .align(Align::Center)
            .fill(token::WELL)
            .child(el().w(self.inset));
        for e in self.leading {
            r = r.child(e);
        }
        r = r.child(
            row()
                .min_w(0.)
                .grow(1.)
                .child(self.centre.unwrap_or_else(el)),
        );
        for e in self.trailing {
            r = r.child(e);
        }
        if self.native_buttons {
            for (suffix, label, glyph, action) in [
                ("minimize", "Minimize", Icon::Remove, WindowAction::Minimize),
                (
                    "maximize",
                    "Maximize or restore",
                    Icon::Window,
                    WindowAction::Maximize,
                ),
                ("close", "Close window", Icon::Close, WindowAction::Close),
            ] {
                let callback = self.action.clone();
                r = r.child(
                    IconButton::new(glyph, label)
                        .key(format!("{}/{suffix}", self.key))
                        .on_press(move |s| callback(s, action))
                        .build()
                        .w(46.)
                        .h(40.),
                );
            }
        }
        r
    }
    /// Register current device-pixel rectangles after every layout/scale change.
    /// Windows answers HTMAXBUTTON for snap-layout hover; caption gaps drag/double-click
    /// through the existing platform backend. No Windows API or idle work is added.
    pub fn sync_chrome(surface: &Surface, runtime: &Runtime<S>, key: &str) {
        let mut regions: Vec<(Rect, Chrome)> = Vec::new();
        if let Some(rect) = runtime.find(key) {
            regions.push((rect, Chrome::Caption));
        }
        for (suffix, kind) in [
            ("minimize", Chrome::Minimize),
            ("maximize", Chrome::Maximize),
            ("close", Chrome::Close),
        ] {
            if let Some(rect) = runtime.find(&format!("{key}/{suffix}")) {
                regions.push((rect, kind));
            }
        }
        surface.set_chrome(&regions);
    }
}
