//! StatusNotifierItem over the session bus; no executor or worker thread.
use super::super::a11y_bus::{Message, pad};
use super::bus::{Bus, string, variant, word};
use crate::tray::{Event, Icon};
use anyhow::{Result, anyhow};
use std::{cell::RefCell, rc::Rc};
const ITEM: &str = "org.kde.StatusNotifierItem";
const PATH: &str = "/StatusNotifierItem";
pub(crate) struct Tray {
    bus: Rc<RefCell<Bus>>,
    id: String,
    tooltip: String,
    icon: Icon,
}
impl Tray {
    pub fn new(id: &str, tooltip: &str, icon: Icon) -> Result<Self> {
        let mut bus = Bus::connect()?;
        let mut body = Vec::new();
        string(
            &mut body,
            "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.kde.StatusNotifierWatcher'",
        );
        let serial = bus.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "AddMatch",
            "s",
            body,
        )?;
        bus.reply(serial)?;
        // Subscribe before registering, so a host appearing now cannot be missed.
        let mut body = Vec::new();
        string(&mut body, PATH);
        bus.call(
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
            "RegisterStatusNotifierItem",
            "s",
            body,
        )?;
        let bus = Rc::new(RefCell::new(bus));
        super::register(&bus);
        Ok(Self {
            bus,
            id: id.into(),
            tooltip: tooltip.into(),
            icon,
        })
    }
    pub fn set_tooltip(&mut self, text: &str) -> Result<()> {
        self.tooltip = text.into();
        self.bus.borrow_mut().signal(PATH, ITEM, "NewToolTip")
    }
    fn property(&self, name: &str, out: &mut Vec<u8>) -> Result<()> {
        match name {
            "Category" => variant(out, "s", |b| string(b, "ApplicationStatus")),
            "Id" => variant(out, "s", |b| string(b, &self.id)),
            "Title" => variant(out, "s", |b| string(b, &self.tooltip)),
            "Status" => variant(out, "s", |b| string(b, "Active")),
            "IconName" => variant(out, "s", |b| string(b, "")),
            "ItemIsMenu" => variant(out, "b", |b| word(b, 0)),
            "IconPixmap" => variant(out, "a(iiay)", |b| self.pixmap(b)),
            "ToolTip" => variant(out, "(sa(iiay)ss)", |b| {
                pad(b, 0, 8);
                string(b, "");
                self.pixmap(b);
                string(b, &self.tooltip);
                string(b, "");
            }),
            _ => return Err(anyhow!("unknown tray property")),
        }
        Ok(())
    }
    fn pixmap(&self, out: &mut Vec<u8>) {
        word(out, 0);
        let size_at = out.len() - 4;
        pad(out, 0, 8);
        let start = out.len();
        word(out, self.icon.width);
        word(out, self.icon.height);
        word(out, self.icon.pixels.len() as u32);
        for p in self.icon.pixels.as_chunks::<4>().0 {
            out.extend_from_slice(&[p[3], p[0], p[1], p[2]]);
        }
        let len = (out.len() - start) as u32;
        out[size_at..size_at + 4].copy_from_slice(&len.to_le_bytes());
    }
    pub fn next_event(&mut self) -> Option<Event> {
        let mut bus = self.bus.borrow_mut();
        bus.dispatch();
        while let Some(bytes) = bus.messages.pop_front() {
            let Some(m) = Message::parse(&bytes) else {
                continue;
            };
            if m.kind == 4
                && m.sender == Some("org.freedesktop.DBus")
                && m.member == Some("NameOwnerChanged")
            {
                if let Some(mut r) = m.body_reader(&bytes, "sss")
                    && r.str() == Some("org.kde.StatusNotifierWatcher")
                    && r.str().is_some()
                    && r.str().is_some_and(|s| !s.is_empty())
                {
                    let mut body = Vec::new();
                    string(&mut body, PATH);
                    let _ = bus.call(
                        "org.kde.StatusNotifierWatcher",
                        "/StatusNotifierWatcher",
                        "org.kde.StatusNotifierWatcher",
                        "RegisterStatusNotifierItem",
                        "s",
                        body,
                    );
                }
                continue;
            }
            if m.kind != 1 || m.path != Some(PATH) {
                continue;
            }
            match (m.interface, m.member) {
                (Some("org.freedesktop.DBus.Peer"), Some("Ping")) => {
                    let _ = bus.answer(&bytes, "", vec![]);
                }
                (Some("org.freedesktop.DBus.Introspectable"), Some("Introspect")) => {
                    let mut out = Vec::new();
                    string(
                        &mut out,
                        r#"<node><interface name="org.kde.StatusNotifierItem"><method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method><method name="ContextMenu"><arg type="i" direction="in"/><arg type="i" direction="in"/></method><method name="SecondaryActivate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method><property name="Category" type="s" access="read"/><property name="Id" type="s" access="read"/><property name="Title" type="s" access="read"/><property name="Status" type="s" access="read"/><property name="IconName" type="s" access="read"/><property name="IconPixmap" type="a(iiay)" access="read"/><property name="ToolTip" type="(sa(iiay)ss)" access="read"/><property name="ItemIsMenu" type="b" access="read"/><signal name="NewToolTip"/></interface></node>"#,
                    );
                    let _ = bus.answer(&bytes, "s", out);
                }
                (Some(ITEM), Some(member @ ("Activate" | "SecondaryActivate" | "ContextMenu"))) => {
                    let Some(mut r) = m.body_reader(&bytes, "ii") else {
                        continue;
                    };
                    let (Some(x), Some(y)) = (r.u32(), r.u32()) else {
                        continue;
                    };
                    let _ = bus.answer(&bytes, "", vec![]);
                    return Some(if member == "ContextMenu" {
                        Event::Right(x as i32, y as i32)
                    } else {
                        Event::Left(x as i32, y as i32)
                    });
                }
                (Some("org.freedesktop.DBus.Properties"), Some("GetAll")) => {
                    let mut out = Vec::new();
                    word(&mut out, 0);
                    pad(&mut out, 0, 8);
                    let start = out.len();
                    for name in [
                        "Category",
                        "Id",
                        "Title",
                        "Status",
                        "IconName",
                        "IconPixmap",
                        "ToolTip",
                        "ItemIsMenu",
                    ] {
                        pad(&mut out, 0, 8);
                        string(&mut out, name);
                        let _ = self.property(name, &mut out);
                    }
                    let len = (out.len() - start) as u32;
                    out[..4].copy_from_slice(&len.to_le_bytes());
                    let _ = bus.answer(&bytes, "a{sv}", out);
                }
                (Some("org.freedesktop.DBus.Properties"), Some("Get")) => {
                    let Some(mut r) = m.body_reader(&bytes, "ss") else {
                        continue;
                    };
                    if r.str() != Some(ITEM) {
                        continue;
                    }
                    let Some(name) = r.str() else { continue };
                    let mut out = Vec::new();
                    if self.property(name, &mut out).is_ok() {
                        let _ = bus.answer(&bytes, "v", out);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
