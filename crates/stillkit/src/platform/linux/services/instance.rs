use super::super::a11y_bus::Message;
use super::bus::{Bus, string, word};
use anyhow::{Result, anyhow, ensure};
use std::{cell::RefCell, rc::Rc};
const INTERFACE: &str = "org.stillkit.Instance";
const PATH: &str = "/org/stillkit/Instance";
pub(crate) struct Instance {
    bus: Rc<RefCell<Bus>>,
}
impl Instance {
    pub fn claim(id: &str, payload: Vec<u8>) -> Result<Option<Self>> {
        let name = format!(
            "org.stillkit.Instance.i{}",
            id.bytes().map(|b| format!("{b:02x}")).collect::<String>()
        );
        ensure!(name.len() <= 255, "instance id too long for D-Bus");
        let mut bus = Bus::connect()?;
        let mut b = Vec::new();
        string(&mut b, &name);
        word(&mut b, 4);
        let n = bus.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "RequestName",
            "su",
            b,
        )?;
        let reply = bus.reply(n)?;
        let status = Message::parse(&reply)
            .and_then(|m| m.body_reader(&reply, "u"))
            .and_then(|mut r| r.u32())
            .ok_or_else(|| anyhow!("invalid name reply"))?;
        if status != 1 {
            ensure!(status == 3, "unexpected instance ownership reply");
            let mut b = Vec::new();
            word(&mut b, payload.len() as u32);
            b.extend(payload);
            let n = bus.call(&name, PATH, INTERFACE, "Activate", "ay", b)?;
            bus.reply(n)?;
            return Ok(None);
        }
        let bus = Rc::new(RefCell::new(bus));
        super::register(&bus);
        Ok(Some(Self { bus }))
    }
    pub fn next_args(&mut self) -> Option<Vec<u8>> {
        let mut bus = self.bus.borrow_mut();
        bus.dispatch();
        while let Some(bytes) = bus.messages.pop_front() {
            let Some(m) = Message::parse(&bytes) else {
                continue;
            };
            if m.kind != 1
                || m.path != Some(PATH)
                || m.interface != Some(INTERFACE)
                || m.member != Some("Activate")
            {
                continue;
            }
            let Some(mut r) = m.body_reader(&bytes, "ay") else {
                continue;
            };
            let Some(len) = r.u32() else { continue };
            if len > 65536 {
                continue;
            }
            let Some(payload) = r.take(len as usize) else {
                continue;
            };
            if crate::instance::decode(payload).is_none() {
                continue;
            }
            let payload = payload.to_vec();
            if bus.answer(&bytes, "", vec![]).is_ok() {
                return Some(payload);
            }
        }
        None
    }
}
