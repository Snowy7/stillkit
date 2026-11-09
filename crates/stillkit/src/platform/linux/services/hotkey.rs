use super::super::a11y_bus::{Message, pad};
use super::{
    bus::{Bus, string, word},
    portal::{self, dict, entry},
};
use anyhow::{Result, anyhow};
use std::{cell::RefCell, rc::Rc};
const INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
pub(crate) struct Hotkey {
    bus: Rc<RefCell<Bus>>,
    session: String,
}
impl Hotkey {
    pub fn new(chord: crate::hotkey::Chord, description: &str) -> Result<Self> {
        let mut bus = Bus::connect()?;
        portal::subscribe(&mut bus, "org.freedesktop.portal.Request")?;
        portal::subscribe(&mut bus, INTERFACE)?;
        let mut b = Vec::new();
        dict(&mut b, |b| {
            entry(b, "handle_token", "s", |b| string(b, "create"));
            entry(b, "session_handle_token", "s", |b| string(b, "hotkey"));
        });
        let response = portal::request(&mut bus, INTERFACE, "CreateSession", "a{sv}", b)?;
        let mut r = Message::parse(&response)
            .and_then(|m| m.body_reader(&response, "ua{sv}"))
            .ok_or_else(|| anyhow!("invalid session"))?;
        r.u32();
        let mut session = None;
        r.array(8, |r| {
            r.align(8)?;
            let key = r.str()?;
            let sig = r.sig()?;
            if key == "session_handle" && matches!(sig, b"s" | b"o") {
                session = Some(r.str()?.to_owned());
            } else {
                r.skip(sig, 0)?;
            }
            Some(())
        })
        .ok_or_else(|| anyhow!("invalid portal session properties"))?;
        let session = session.ok_or_else(|| anyhow!("portal omitted session_handle"))?;
        let mut b = Vec::new();
        string(&mut b, &session);
        word(&mut b, 0);
        let at = b.len() - 4;
        pad(&mut b, 0, 8);
        let start = b.len();
        string(&mut b, "hotkey");
        dict(&mut b, |b| {
            entry(b, "description", "s", |b| string(b, description));
            entry(b, "preferred_trigger", "s", |b| string(b, &chord.trigger));
        });
        let len = (b.len() - start) as u32;
        b[at..at + 4].copy_from_slice(&len.to_le_bytes());
        string(&mut b, "");
        dict(&mut b, |b| {
            entry(b, "handle_token", "s", |b| string(b, "bind"))
        });
        let hotkey = Self {
            bus: Rc::new(RefCell::new(bus)),
            session,
        };
        let response = portal::request(
            &mut hotkey.bus.borrow_mut(),
            INTERFACE,
            "BindShortcuts",
            "oa(sa{sv})sa{sv}",
            b,
        )?;
        let mut r = Message::parse(&response)
            .and_then(|m| m.body_reader(&response, "ua{sv}"))
            .ok_or_else(|| anyhow!("invalid binding reply"))?;
        r.u32();
        let mut bound = false;
        r.array(8, |r| {
            r.align(8)?;
            let key = r.str()?;
            let sig = r.sig()?;
            if key == "shortcuts" && sig == b"a(sa{sv})" {
                r.array(8, |r| {
                    r.align(8)?;
                    bound |= r.str()? == "hotkey";
                    r.skip(b"a{sv}", 0)?;
                    Some(())
                })?;
            } else {
                r.skip(sig, 0)?;
            }
            Some(())
        })
        .ok_or_else(|| anyhow!("invalid portal binding properties"))?;
        anyhow::ensure!(bound, "portal did not bind the shortcut");
        super::register(&hotkey.bus);
        Ok(hotkey)
    }
    pub fn activated(&mut self) -> bool {
        let mut bus = self.bus.borrow_mut();
        bus.dispatch();
        while let Some(bytes) = bus.messages.pop_front() {
            let Some(m) = Message::parse(&bytes) else {
                continue;
            };
            if m.kind == 4 && m.interface == Some(INTERFACE) && m.member == Some("Activated")
                && let Some(mut r) = m.body_reader(&bytes, "osta{sv}")
                    && r.str() == Some(self.session.as_str()) && r.str() == Some("hotkey") {
                        return true;
                    }
        }
        false
    }
}
impl Drop for Hotkey {
    fn drop(&mut self) {
        let _ = self.bus.borrow_mut().call(
            portal::DEST,
            &self.session,
            "org.freedesktop.portal.Session",
            "Close",
            "",
            vec![],
        );
    }
}
