//! NSStatusItem integration is reserved; unsupported calls allocate no native resources.
pub(crate) struct Tray;
impl Tray {
    pub fn new(_: &str, _: &str, _: crate::tray::Icon) -> anyhow::Result<Self> {
        anyhow::bail!("NSStatusItem is not implemented")
    }
    pub fn next_event(&mut self) -> Option<crate::tray::Event> {
        None
    }
    pub fn set_tooltip(&mut self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("NSStatusItem is not implemented")
    }
}

pub(crate) struct Hotkey;
impl Hotkey {
 pub fn new(_:crate::hotkey::Chord,_:&str)->anyhow::Result<Self> { anyhow::bail!("global hotkeys are unsupported") }
 pub fn activated(&mut self)->bool { false }
}

pub(crate) struct Notifications;
impl Notifications {
 pub fn new(_:&str,_:crate::tray::Icon)->anyhow::Result<Self> {anyhow::bail!("notifications unsupported")}
 pub fn show(&mut self,_:&str,_:&str)->anyhow::Result<u32> {anyhow::bail!("notifications unsupported")}
 pub fn next_action(&mut self)->Option<crate::notify::Action> {None}
}

pub(crate) struct Instance;
impl Instance {
 pub fn claim(_:&str,_:Vec<u8>)->anyhow::Result<Option<Self>> {anyhow::bail!("single instance unsupported")}
 pub fn next_args(&mut self)->Option<Vec<u8>> {None}
}
