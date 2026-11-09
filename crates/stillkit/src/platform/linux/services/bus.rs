//! Session-bus transport, sharing the AT-SPI address parser and bounded wire reader.
use super::super::a11y_bus::{self, Message, pad, put_str};
use anyhow::{Result, anyhow, bail};
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::{Duration, Instant},
};
pub(crate) struct Bus {
    stream: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
    serial: u32,
    pub messages: VecDeque<Vec<u8>>,
    dead: bool,
    pub name: String,
}
impl Bus {
    pub fn connect() -> Result<Self> {
        let addresses = std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .map(|s| a11y_bus::addresses(&s))
            .unwrap_or_else(|_| {
                use std::os::unix::ffi::OsStrExt;
                std::env::var_os("XDG_RUNTIME_DIR")
                    .map(|p| vec![a11y_bus::Address::Path([p.as_bytes(), b"/bus"].concat())])
                    .unwrap_or_default()
            });
        let mut stream = addresses
            .iter()
            .find_map(super::super::a11y::open)
            .ok_or_else(|| anyhow!("no session bus"))?;
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        // SAFETY: getuid is a process query with no preconditions.
        let hex: String = unsafe { libc::getuid() }
            .to_string()
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect();
        stream.write_all(format!("\0AUTH EXTERNAL {hex}\r\n").as_bytes())?;
        let mut line = Vec::new();
        while !line.ends_with(b"\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte)?;
            line.push(byte[0]);
            if line.len() > 4096 {
                bail!("D-Bus authentication too long")
            }
        }
        if !line.starts_with(b"OK ") {
            bail!("D-Bus authentication rejected")
        }
        stream.write_all(b"BEGIN\r\n")?;
        stream.set_nonblocking(true)?;
        let mut bus = Self {
            stream,
            input: Vec::new(),
            output: Vec::new(),
            serial: 0,
            messages: VecDeque::new(),
            dead: false,
            name: String::new(),
        };
        let serial = bus.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "Hello",
            "",
            vec![],
        )?;
        let bytes = bus.reply(serial)?;
        bus.name = Message::parse(&bytes)
            .and_then(|m| m.body_reader(&bytes, "s"))
            .and_then(|mut r| r.str())
            .ok_or_else(|| anyhow!("invalid bus name"))?
            .into();
        Ok(bus)
    }
    #[allow(clippy::too_many_arguments)] // D-Bus method addressing and body.
    pub fn call(
        &mut self,
        dest: &str,
        path: &str,
        interface: &str,
        member: &str,
        signature: &str,
        body: Vec<u8>,
    ) -> Result<u32> {
        self.serial += 1;
        let serial = self.serial;
        self.send(
            1,
            serial,
            vec![
                (1, b'o', path),
                (2, b's', interface),
                (3, b's', member),
                (6, b's', dest),
                (8, b'g', signature),
            ],
            None,
            body,
        )?;
        Ok(serial)
    }
    pub fn signal(&mut self, path: &str, interface: &str, member: &str) -> Result<()> {
        self.serial += 1;
        self.send(
            4,
            self.serial,
            vec![(1, b'o', path), (2, b's', interface), (3, b's', member)],
            None,
            vec![],
        )
    }
    pub fn answer(&mut self, request: &[u8], signature: &str, body: Vec<u8>) -> Result<()> {
        let m = Message::parse(request).ok_or_else(|| anyhow!("invalid method"))?;
        let sender = m.sender.ok_or_else(|| anyhow!("missing sender"))?;
        let serial = if request[0] == b'l' {
            u32::from_le_bytes(request[8..12].try_into()?)
        } else {
            u32::from_be_bytes(request[8..12].try_into()?)
        };
        self.serial += 1;
        self.send(
            2,
            self.serial,
            vec![(6, b's', sender), (8, b'g', signature)],
            Some(serial),
            body,
        )
    }
    fn send(
        &mut self,
        kind: u8,
        serial: u32,
        fields: Vec<(u8, u8, &str)>,
        reply: Option<u32>,
        body: Vec<u8>,
    ) -> Result<()> {
        let out = super::wire::encode(kind, serial, fields, reply, body);
        self.output.extend(out);
        self.flush()
    }
    fn flush(&mut self) -> Result<()> {
        while !self.output.is_empty() {
            match self.stream.write(&self.output) {
                Ok(0) => bail!("session bus closed"),
                Ok(n) => {
                    self.output.drain(..n);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    pub fn poll_fd(&self) -> Option<libc::pollfd> {
        (!self.dead).then_some(libc::pollfd {
            fd: self.stream.as_raw_fd(),
            events: libc::POLLIN
                | if self.output.is_empty() {
                    0
                } else {
                    libc::POLLOUT
                },
            revents: 0,
        })
    }
    pub fn dispatch(&mut self) -> bool {
        if self.dead {
            return false;
        }
        if let Err(e) = self.read() {
            log::warn!("desktop service: {e}");
            self.dead = true;
        }
        !self.messages.is_empty()
    }
    fn read(&mut self) -> Result<()> {
        self.flush()?;
        let mut chunk = [0; 8192];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => bail!("session bus closed"),
                Ok(n) => self.input.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
            if self.input.len() > 4 * 1024 * 1024 {
                bail!("desktop service message limit")
            }
        }
        while let Some(len) =
            a11y_bus::message_len(&self.input).map_err(|_| anyhow!("invalid D-Bus frame"))?
        {
            if len > 4 * 1024 * 1024 {
                bail!("desktop service message limit")
            }
            if self.input.len() < len {
                break;
            }
            if self.messages.len() >= 256 {
                bail!("desktop service queue limit")
            }
            self.messages.push_back(self.input.drain(..len).collect());
        }
        Ok(())
    }
    pub fn reply(&mut self, serial: u32) -> Result<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.read()?;
            if let Some(i) = self
                .messages
                .iter()
                .position(|b| Message::parse(b).is_some_and(|m| m.reply_serial == Some(serial)))
            {
                let bytes = self.messages.remove(i).unwrap();
                if bytes[1] == 3 {
                    bail!("desktop service rejected request")
                }
                return Ok(bytes);
            }
            self.wait(Some(deadline))?;
        }
    }
    pub fn wait(&self, deadline: Option<Instant>) -> Result<()> {
        let mut fd = self.poll_fd().ok_or_else(|| anyhow!("bus closed"))?;
        let timeout = if let Some(at) = deadline {
            let remaining = at.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("desktop service timed out")
            }
            remaining.as_millis().min(i32::MAX as u128) as i32
        } else {
            -1
        };
        // SAFETY: fd is initialized and stays owned throughout the wait.
        if unsafe { libc::poll(&mut fd, 1, timeout) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
}
pub(crate) fn string(out: &mut Vec<u8>, value: &str) {
    pad(out, 0, 4);
    put_str(out, value);
}
pub(crate) fn word(out: &mut Vec<u8>, value: u32) {
    pad(out, 0, 4);
    out.extend_from_slice(&value.to_le_bytes());
}
pub(crate) fn variant(out: &mut Vec<u8>, signature: &str, value: impl FnOnce(&mut Vec<u8>)) {
    out.push(signature.len() as u8);
    out.extend_from_slice(signature.as_bytes());
    out.push(0);
    value(out);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variant_dictionary_alignment_matches_dbus() {
        let mut bytes = Vec::new();
        word(&mut bytes, 0);
        pad(&mut bytes, 0, 8);
        let start = bytes.len();
        pad(&mut bytes, 0, 8);
        string(&mut bytes, "multiple");
        variant(&mut bytes, "b", |b| word(b, 1));
        let len = (bytes.len() - start) as u32;
        bytes[..4].copy_from_slice(&len.to_le_bytes());
        assert_eq!(bytes[4..8], [0; 4]);
        assert_eq!(bytes[8..12], 8u32.to_le_bytes());
        assert_eq!(&bytes[12..21], b"multiple\0");
        assert_eq!(&bytes[21..24], b"\x01b\0");
        assert_eq!(&bytes[24..28], &1u32.to_le_bytes());
    }
}
