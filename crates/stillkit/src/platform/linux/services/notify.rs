use super::super::a11y_bus::Message;
use super::{
    bus::{Bus, string, word},
    portal::dict,
};
use anyhow::{Result, anyhow};
use std::{cell::RefCell, collections::HashSet, rc::Rc};
const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
pub(crate) struct Notifications {
    bus: Rc<RefCell<Bus>>,
    app: String,
    ids: HashSet<u32>,
}
impl Notifications {
    pub fn new(id: &str, _: crate::tray::Icon) -> Result<Self> {
        let mut bus = Bus::connect()?;
        let mut b = Vec::new();
        string(
            &mut b,
            "type='signal',sender='org.freedesktop.Notifications',interface='org.freedesktop.Notifications'",
        );
        let n = bus.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "AddMatch",
            "s",
            b,
        )?;
        bus.reply(n)?;
        let bus = Rc::new(RefCell::new(bus));
        super::register(&bus);
        Ok(Self {
            bus,
            app: id.into(),
            ids: HashSet::new(),
        })
    }
    pub fn show(&mut self, title: &str, body: &str) -> Result<u32> {
        let mut b = Vec::new();
        string(&mut b, &self.app);
        word(&mut b, 0);
        string(&mut b, "");
        string(&mut b, title);
        string(&mut b, body);
        let mut actions = Vec::new();
        string(&mut actions, "default");
        string(&mut actions, "Open");
        word(&mut b, actions.len() as u32);
        b.extend(actions);
        dict(&mut b, |_| {});
        word(&mut b, u32::MAX);
        let mut bus = self.bus.borrow_mut();
        let n = bus.call(DEST, PATH, DEST, "Notify", "susssasa{sv}i", b)?;
        let bytes = bus.reply(n)?;
        let id = Message::parse(&bytes)
            .and_then(|m| m.body_reader(&bytes, "u"))
            .and_then(|mut r| r.u32())
            .ok_or_else(|| anyhow!("invalid notification identifier"))?;
        self.ids.insert(id);
        Ok(id)
    }
    pub fn next_action(&mut self) -> Option<crate::notify::Action> {
        let mut bus = self.bus.borrow_mut();
        bus.dispatch();
        while let Some(bytes) = bus.messages.pop_front() {
            let Some(m) = Message::parse(&bytes) else {
                continue;
            };
            if m.kind == 4
                && m.interface == Some(DEST)
                && m.path == Some(PATH)
                && m.member == Some("NotificationClosed")
            {
                if let Some(mut r) = m.body_reader(&bytes, "uu")
                    && let Some(id) = r.u32() {
                        self.ids.remove(&id);
                    }
                continue;
            }
            if m.kind != 4
                || m.interface != Some(DEST)
                || m.member != Some("ActionInvoked")
                || m.path != Some(PATH)
            {
                continue;
            }
            if let Some(mut r) = m.body_reader(&bytes, "us")
                && let (Some(id), Some(action)) = (r.u32(), r.str())
                    && self.ids.contains(&id) {
                        return Some(crate::notify::Action {
                            id,
                            action: action.into(),
                        });
                    }
        }
        None
    }
}
impl Drop for Notifications {
    fn drop(&mut self) {
        let mut bus = self.bus.borrow_mut();
        for id in &self.ids {
            let mut b = Vec::new();
            word(&mut b, *id);
            let _ = bus.call(DEST, PATH, DEST, "CloseNotification", "u", b);
        }
    }
}
