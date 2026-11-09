//! Portal request/response exchange; waits on the bus fd, never a polling thread.
use super::super::a11y_bus::{Message, pad};
use super::bus::{Bus, string, variant, word};
use anyhow::{Result, anyhow, ensure};
use std::time::{Duration, Instant};
pub(crate) const DEST: &str = "org.freedesktop.portal.Desktop";
pub(crate) const PATH: &str = "/org/freedesktop/portal/desktop";
pub(crate) fn subscribe(bus: &mut Bus, interface: &str) -> Result<()> {
    let mut b = Vec::new();
    string(
        &mut b,
        &format!("type='signal',sender='{DEST}',interface='{interface}'"),
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
    Ok(())
}
pub(crate) fn dict(out: &mut Vec<u8>, f: impl FnOnce(&mut Vec<u8>)) {
    word(out, 0);
    let at = out.len() - 4;
    pad(out, 0, 8);
    let start = out.len();
    f(out);
    let len = (out.len() - start) as u32;
    out[at..at + 4].copy_from_slice(&len.to_le_bytes());
}
pub(crate) fn entry(
    out: &mut Vec<u8>,
    name: &str,
    signature: &str,
    value: impl FnOnce(&mut Vec<u8>),
) {
    pad(out, 0, 8);
    string(out, name);
    variant(out, signature, value);
}
pub(crate) fn request(
    bus: &mut Bus,
    interface: &str,
    method: &str,
    signature: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>> {
    let n = bus.call(DEST, PATH, interface, method, signature, body)?;
    let reply = bus.reply(n)?;
    let path = Message::parse(&reply)
        .and_then(|m| m.body_reader(&reply, "o"))
        .and_then(|mut r| r.str())
        .ok_or_else(|| anyhow!("invalid portal request path"))?
        .to_owned();
    let deadline = Instant::now() + Duration::from_secs(120);
    let result = (|| {
        loop {
            bus.dispatch();
            while let Some(bytes) = bus.messages.pop_front() {
                let Some(m) = Message::parse(&bytes) else {
                    continue;
                };
                if m.kind != 4
                    || m.path != Some(path.as_str())
                    || m.interface != Some("org.freedesktop.portal.Request")
                    || m.member != Some("Response")
                {
                    continue;
                }
                let mut r = m
                    .body_reader(&bytes, "ua{sv}")
                    .ok_or_else(|| anyhow!("invalid portal response"))?;
                ensure!(r.u32() == Some(0), "portal request cancelled or denied");
                return Ok(bytes);
            }
            bus.wait(Some(deadline))?;
        }
    })();
    if result.is_err() {
        let _ = bus.call(
            DEST,
            &path,
            "org.freedesktop.portal.Request",
            "Close",
            "",
            vec![],
        );
    }
    result
}
