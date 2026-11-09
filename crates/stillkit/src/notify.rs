//! Opt-in desktop notifications. Windows uses a small tray icon and shell balloons,
//! so it needs no WinRT runtime. Linux uses org.freedesktop.Notifications.
use anyhow::Result;
/// A notification's action was clicked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    /// The identifier returned by `Notifications::show`.
    pub id: u32,
    /// The desktop action key (`default` for Windows balloons).
    pub action: String,
}
/// Owns notification delivery on the UI thread. Drop closes owned notifications.
/// Windows displays the supplied tray icon for this object's lifetime.
pub struct Notifications(crate::platform::current::services::Notifications);
impl Notifications {
    /// Connects to the desktop on first use; the icon is used by Windows balloons.
    pub fn new(app_id: &str, icon: crate::tray::Icon) -> Result<Self> {
        crate::platform::current::services::Notifications::new(app_id, icon).map(Self)
    }
    /// Shows a notification with a default action. Windows accepts one active balloon at a time.
    pub fn show(&mut self, title: &str, body: &str) -> Result<u32> {
        self.0.show(title, body)
    }
    /// Takes an action after native dispatch.
    pub fn next_action(&mut self) -> Option<Action> {
        self.0.next_action()
    }
}
