//! Whether assistive technology is on: the `org.a11y.Status` properties, read the way Qt does
//! (`IsEnabled || ScreenReaderEnabled`), over a hand-written session-bus client. Pure bytes in,
//! bytes and decisions out, so it is unit-tested on every host; `a11y.rs` owns the socket and
//! puts it in the Wayland poll. A connection sends one pipelined write (SASL EXTERNAL, `BEGIN`,
//! `Hello`, `AddMatch`, `GetAll`) and then only reads, waking only when the bus sends us something.

/// `STILLKIT_ACCESSIBILITY`: `1` forces AT-SPI on, `0` forces it off, unset (or anything else)
/// follows `org.a11y.Status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    On,
    Off,
    Auto,
}
impl Mode {
    pub(crate) fn from_env(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("1") => Mode::On,
            Some("0") => Mode::Off,
            _ => Mode::Auto,
        }
    }
}

/// A session-bus socket address from `DBUS_SESSION_BUS_ADDRESS`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Address {
    Path(Vec<u8>),
    Abstract(Vec<u8>),
}
/// The `unix:` addresses in a D-Bus address list, in order. Other transports are skipped; with
/// no variable at all, callers fall back to `$XDG_RUNTIME_DIR/bus`, as sd-bus and GDBus do.
pub(crate) fn addresses(list: &str) -> Vec<Address> {
    list.split(';')
        .filter_map(|entry| {
            let keys = entry.strip_prefix("unix:")?;
            keys.split(',').find_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                let value = unescape(value)?;
                match key {
                    "path" => Some(Address::Path(value)),
                    "abstract" => Some(Address::Abstract(value)),
                    _ => None,
                }
            })
        })
        .collect()
}
fn unescape(value: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hex = [bytes.next()?, bytes.next()?];
            out.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            out.push(b);
        }
    }
    Some(out)
}

const BUS: &str = "org.a11y.Bus";
const PATH: &str = "/org/a11y/bus";
const STATUS: &str = "org.a11y.Status";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const MATCH: &str = "type='signal',sender='org.a11y.Bus',interface='org.freedesktop.DBus.Properties',\
member='PropertiesChanged',path='/org/a11y/bus',arg0='org.a11y.Status'";
/// The status service restarting under a new unique name, or exiting.
const OWNERS: &str = "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',\
member='NameOwnerChanged',path='/org/freedesktop/DBus',arg0='org.a11y.Bus'";
const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
/// The largest message kept in memory; bigger ones (never sent by the status service) are
/// read past and dropped.
const MAX_MESSAGE: usize = 64 * 1024;
const NO_REPLY_EXPECTED: u8 = 0x1;
const NO_AUTO_START: u8 = 0x2;

/// The connection was refused, broke, or the status service is absent: close the socket.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Gone;

/// One session-bus conversation about `org.a11y.Status`.
pub(crate) struct Session {
    input: Vec<u8>,
    authed: bool,
    discard: usize,
    serial: u32,
    get_all: u32,
    owner: Option<String>,
    is_enabled: bool,
    screen_reader: bool,
    known: bool,
    output: Vec<u8>,
}
impl Session {
    /// Starts a conversation for `uid`; `take_output()` holds everything to write first. The
    /// status call has NO_AUTO_START, so a missing service is not started on our behalf.
    pub(crate) fn start(uid: u32) -> Self {
        let mut output = Vec::with_capacity(512);
        output.push(0);
        let hex: String = uid.to_string().bytes().map(|b| format!("{b:02x}")).collect();
        output.extend_from_slice(format!("AUTH EXTERNAL {hex}\r\nBEGIN\r\n").as_bytes());
        let mut session = Self {
            input: Vec::new(),
            authed: false,
            discard: 0,
            serial: 0,
            get_all: 0,
            owner: None,
            is_enabled: false,
            screen_reader: false,
            known: false,
            output,
        };
        session.call(DBUS, DBUS_PATH, DBUS, "Hello", None, 0);
        session.call(DBUS, DBUS_PATH, DBUS, "AddMatch", Some(MATCH), NO_REPLY_EXPECTED);
        session.call(DBUS, DBUS_PATH, DBUS, "AddMatch", Some(OWNERS), NO_REPLY_EXPECTED);
        session.request();
        session
    }
    /// Bytes to write now; empty when there is nothing to send.
    pub(crate) fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }
    /// Whether assistive technology is on, once the service has answered.
    pub(crate) fn enabled(&self) -> Option<bool> {
        self.known.then_some(self.is_enabled || self.screen_reader)
    }
    /// Consumes bytes read from the socket.
    pub(crate) fn feed(&mut self, mut bytes: &[u8]) -> Result<(), Gone> {
        let skip = self.discard.min(bytes.len());
        self.discard -= skip;
        bytes = &bytes[skip..];
        self.input.extend_from_slice(bytes);
        if !self.authed {
            let Some(end) = self.input.windows(2).position(|w| w == b"\r\n") else {
                return if self.input.len() > 4096 { Err(Gone) } else { Ok(()) };
            };
            if !self.input.starts_with(b"OK ") {
                return Err(Gone);
            }
            self.input.drain(..end + 2);
            self.authed = true;
        }
        loop {
            let Some(len) = message_len(&self.input)? else { return Ok(()) };
            if len > MAX_MESSAGE {
                self.discard = len - self.input.len().min(len);
                self.input.drain(..self.input.len().min(len));
                if self.discard > 0 {
                    return Ok(());
                }
                continue;
            }
            if self.input.len() < len {
                return Ok(());
            }
            let message: Vec<u8> = self.input.drain(..len).collect();
            self.handle(&message)?;
        }
    }
    fn handle(&mut self, bytes: &[u8]) -> Result<(), Gone> {
        let m = Message::parse(bytes).ok_or(Gone)?;
        match m.kind {
            METHOD_RETURN if m.reply_serial == Some(self.get_all) => {
                let mut r = m.body_reader(bytes, "a{sv}").ok_or(Gone)?;
                self.apply(&mut r).ok_or(Gone)?;
                self.owner = m.sender.map(str::to_owned);
                self.known = true;
            }
            // Absent at startup: give up. Later (a restart race): off until a new owner appears.
            ERROR if m.reply_serial == Some(self.get_all) => {
                if !self.known {
                    return Err(Gone);
                }
                self.off();
            }
            SIGNAL if m.member == Some("NameOwnerChanged")
                && m.interface == Some(DBUS)
                && m.path == Some(DBUS_PATH)
                && m.sender == Some(DBUS) =>
            {
                let mut r = m.body_reader(bytes, "sss").ok_or(Gone)?;
                let (name, _old, new) = (r.str().ok_or(Gone)?, r.str().ok_or(Gone)?, r.str().ok_or(Gone)?);
                if name != BUS || !self.known {
                    return Ok(());
                }
                self.off();
                if !new.is_empty() {
                    self.request();
                }
            }
            SIGNAL if m.member == Some("PropertiesChanged")
                && m.interface == Some(PROPERTIES)
                && m.path == Some(PATH)
                && self.owner.is_some()
                && m.sender == self.owner.as_deref() =>
            {
                let mut r = m.body_reader(bytes, "sa{sv}as").ok_or(Gone)?;
                if r.str().ok_or(Gone)? != STATUS {
                    return Ok(());
                }
                self.apply(&mut r).ok_or(Gone)?;
                let mut invalidated = false;
                r.array(4, |r| {
                    invalidated |= matches!(r.str()?, "IsEnabled" | "ScreenReaderEnabled");
                    Some(())
                })
                .ok_or(Gone)?;
                if invalidated {
                    self.request();
                }
            }
            _ => {}
        }
        Ok(())
    }
    /// The service is gone: no assistive technology can reach us.
    fn off(&mut self) {
        self.owner = None;
        self.is_enabled = false;
        self.screen_reader = false;
    }
    /// Reads an `a{sv}` of properties, keeping the two booleans and skipping anything else.
    fn apply(&mut self, r: &mut Reader) -> Option<()> {
        r.array(8, |r| {
            r.align(8)?;
            let name = r.str()?;
            let sig = r.sig()?;
            if sig == b"b" {
                let value = r.u32()? != 0;
                match name {
                    "IsEnabled" => self.is_enabled = value,
                    "ScreenReaderEnabled" => self.screen_reader = value,
                    _ => {}
                }
            } else if !r.skip(sig, 0)?.is_empty() {
                return None;
            }
            Some(())
        })
    }
    fn request(&mut self) {
        self.get_all = self.call(BUS, PATH, PROPERTIES, "GetAll", Some(STATUS), NO_AUTO_START);
    }
    fn call(&mut self, dest: &str, path: &str, iface: &str, member: &str, arg: Option<&str>, flags: u8) -> u32 {
        self.serial += 1;
        let serial = self.serial;
        let mut body = Vec::new();
        if let Some(arg) = arg {
            put_str(&mut body, arg);
        }
        let start = self.output.len();
        let out = &mut self.output;
        out.extend_from_slice(&[b'l', METHOD_CALL, flags, 1]);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&serial.to_le_bytes());
        let fields_at = out.len();
        out.extend_from_slice(&[0; 4]);
        let mut fields = vec![(1, b'o', path), (2, b's', iface), (3, b's', member), (6, b's', dest)];
        if arg.is_some() {
            fields.push((8, b'g', "s"));
        }
        for (code, sig, value) in fields {
            pad(out, start, 8);
            out.extend_from_slice(&[code, 1, sig, 0]);
            if sig == b'g' {
                out.push(value.len() as u8);
                out.extend_from_slice(value.as_bytes());
                out.push(0);
            } else {
                put_str(out, value);
            }
        }
        let fields_len = (out.len() - fields_at - 4) as u32;
        out[fields_at..fields_at + 4].copy_from_slice(&fields_len.to_le_bytes());
        pad(out, start, 8);
        out.extend_from_slice(&body);
        serial
    }
}
pub(super) fn pad(out: &mut Vec<u8>, start: usize, to: usize) {
    while !(out.len() - start).is_multiple_of(to) {
        out.push(0);
    }
}
/// A string at a 4-aligned offset (every caller's position is).
pub(super) fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

const METHOD_CALL: u8 = 1;
const METHOD_RETURN: u8 = 2;
const ERROR: u8 = 3;
const SIGNAL: u8 = 4;

/// The full length of the message at the start of `bytes`, once its fixed header has arrived.
pub(super) fn message_len(bytes: &[u8]) -> Result<Option<usize>, Gone> {
    if bytes.len() < 16 {
        return Ok(None);
    }
    let big = match bytes[0] {
        b'l' => false,
        b'B' => true,
        _ => return Err(Gone),
    };
    if bytes[3] != 1 {
        return Err(Gone);
    }
    let read = |at: usize| {
        let word = [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
        (if big { u32::from_be_bytes(word) } else { u32::from_le_bytes(word) }) as usize
    };
    let fields = (16 + read(12)).next_multiple_of(8);
    // The D-Bus limit is 128 MiB; anything larger is a broken stream.
    fields.checked_add(read(4)).filter(|&len| len <= 1 << 27).map(Some).ok_or(Gone)
}

/// The header fields this client reads.
pub(super) struct Message<'a> {
    pub(super) kind: u8,
    pub(super) big: bool,
    pub(super) body_at: usize,
    pub(super) reply_serial: Option<u32>,
    pub(super) path: Option<&'a str>,
    pub(super) interface: Option<&'a str>,
    pub(super) member: Option<&'a str>,
    pub(super) sender: Option<&'a str>,
    pub(super) signature: &'a [u8],
}
impl<'a> Message<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Option<Self> {
        let big = bytes[0] == b'B';
        let mut r = Reader { bytes, at: 12, big };
        let mut m = Message {
            kind: bytes[1],
            big,
            body_at: 0,
            reply_serial: None,
            path: None,
            interface: None,
            member: None,
            sender: None,
            signature: b"",
        };
        r.array(8, |r| {
            r.align(8)?;
            let code = r.u8()?;
            let sig = r.sig()?;
            match (code, sig) {
                (1, b"o") => m.path = Some(r.str()?),
                (2, b"s") => m.interface = Some(r.str()?),
                (3, b"s") => m.member = Some(r.str()?),
                (5, b"u") => m.reply_serial = Some(r.u32()?),
                (7, b"s") => m.sender = Some(r.str()?),
                (8, b"g") => m.signature = r.sig()?,
                _ => {
                    if !r.skip(sig, 0)?.is_empty() {
                        return None;
                    }
                }
            }
            Some(())
        })?;
        r.align(8)?;
        m.body_at = r.at;
        Some(m)
    }
    /// A reader at the body, if its signature is `expected`.
    pub(super) fn body_reader(&self, bytes: &'a [u8], expected: &str) -> Option<Reader<'a>> {
        (self.signature == expected.as_bytes()).then_some(Reader { bytes, at: self.body_at, big: self.big })
    }
}

/// Reads D-Bus wire values; positions are absolute in the message, as alignment requires.
/// Every method returns `None` on malformed or truncated input.
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    big: bool,
}
impl<'a> Reader<'a> {
    pub(super) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let out = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(out)
    }
    pub(super) fn align(&mut self, to: usize) -> Option<()> {
        let at = self.at.next_multiple_of(to);
        self.take(at - self.at).map(|_| ())
    }
    pub(super) fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub(super) fn u32(&mut self) -> Option<u32> {
        self.align(4)?;
        let word: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(if self.big { u32::from_be_bytes(word) } else { u32::from_le_bytes(word) })
    }
    pub(super) fn str(&mut self) -> Option<&'a str> {
        let len = self.u32()? as usize;
        let s = self.take(len.checked_add(1)?)?;
        std::str::from_utf8(&s[..len]).ok()
    }
    pub(super) fn sig(&mut self) -> Option<&'a [u8]> {
        let len = self.u8()? as usize;
        Some(&self.take(len + 1)?[..len])
    }
    /// Calls `each` for every element of an array whose elements align to `to`; together
    /// they must consume exactly its length (which excludes the padding before the first).
    pub(super) fn array(&mut self, to: usize, mut each: impl FnMut(&mut Self) -> Option<()>) -> Option<()> {
        let len = self.u32()? as usize;
        self.align(to)?;
        let end = self.at.checked_add(len)?;
        if end > self.bytes.len() {
            return None;
        }
        while self.at < end {
            each(self)?;
        }
        (self.at == end).then_some(())
    }
    /// Skips one complete value of the type that starts `sig`; returns the rest of `sig`.
    pub(super) fn skip<'s>(&mut self, sig: &'s [u8], depth: u8) -> Option<&'s [u8]> {
        if depth > 64 {
            return None;
        }
        let (&code, rest) = sig.split_first()?;
        match code {
            b'y' => {
                self.take(1)?;
            }
            b'n' | b'q' => {
                self.align(2)?;
                self.take(2)?;
            }
            b'b' | b'i' | b'u' | b'h' => {
                self.u32()?;
            }
            b'x' | b't' | b'd' => {
                self.align(8)?;
                self.take(8)?;
            }
            b's' | b'o' => {
                self.str()?;
            }
            b'g' => {
                self.sig()?;
            }
            b'v' => {
                let inner = self.sig()?;
                if !self.skip(inner, depth + 1)?.is_empty() {
                    return None;
                }
            }
            b'a' => {
                let len = self.u32()? as usize;
                let element = type_len(rest)?;
                self.align(alignment(rest[0]))?;
                self.take(len)?;
                return Some(&rest[element..]);
            }
            b'(' | b'{' => {
                self.align(8)?;
                let close = if code == b'(' { b')' } else { b'}' };
                let mut fields = rest;
                while *fields.first()? != close {
                    fields = self.skip(fields, depth + 1)?;
                }
                return Some(&fields[1..]);
            }
            _ => return None,
        }
        Some(rest)
    }
}
/// The length of the first complete type in `sig`.
fn type_len(sig: &[u8]) -> Option<usize> {
    let mut depth = 0i32;
    for (i, &code) in sig.iter().enumerate() {
        match code {
            b'a' => continue,
            b'(' | b'{' => depth += 1,
            b')' | b'}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return None;
        }
        if depth == 0 {
            return Some(i + 1);
        }
    }
    None
}
fn alignment(code: u8) -> usize {
    match code {
        b'n' | b'q' => 2,
        b'b' | b'i' | b'u' | b'h' | b's' | b'o' | b'a' => 4,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use zbus::message::{Message as Zmsg, Type};
    use zbus::zvariant::{serialized::{Context, Data}, Endian, Value};

    const OK: &[u8] = b"OK 1234deadbeef\r\n";
    const OWNER: &str = ":1.7";

    /// Splits our pipelined output into the auth lines and zbus-parsed messages.
    fn sent(bytes: &[u8]) -> (String, Vec<Zmsg>) {
        let begin = bytes.windows(7).position(|w| w == b"BEGIN\r\n").unwrap() + 7;
        let auth = String::from_utf8(bytes[..begin].to_vec()).unwrap();
        let mut rest = &bytes[begin..];
        let mut messages = Vec::new();
        while !rest.is_empty() {
            let len = message_len(rest).unwrap().unwrap();
            let data = Data::new(rest[..len].to_vec(), Context::new_dbus(Endian::Little, 0));
            // SAFETY: the bytes carry no file descriptors (UNIX_FDS is never set).
            messages.push(unsafe { Zmsg::from_bytes(data) }.unwrap());
            rest = &rest[len..];
        }
        (auth, messages)
    }
    fn reply(call: &Zmsg, endian: Endian, props: &HashMap<&str, Value>) -> Vec<u8> {
        Zmsg::method_return(&call.header()).unwrap().sender(OWNER).unwrap().endian(endian).build(props).unwrap().data().to_vec()
    }
    fn changed(sender: &str, changed: &HashMap<&str, Value>, invalidated: &[&str]) -> Vec<u8> {
        Zmsg::signal(PATH, PROPERTIES, "PropertiesChanged").unwrap().sender(sender).unwrap()
            .build(&(STATUS, changed, invalidated)).unwrap().data().to_vec()
    }
    fn started() -> (Session, Zmsg) {
        let mut s = Session::start(1000);
        let (_, mut sent) = sent(&s.take_output());
        (s, sent.pop().unwrap())
    }

    #[test]
    fn overrides_and_addresses() {
        assert_eq!(Mode::from_env(Some("1")), Mode::On);
        assert_eq!(Mode::from_env(Some("0")), Mode::Off);
        assert_eq!(Mode::from_env(None), Mode::Auto);
        assert_eq!(Mode::from_env(Some("yes")), Mode::Auto);
        assert_eq!(
            addresses("tcp:host=x,port=1;unix:abstract=/tmp/dbus-a%2cb,guid=00;unix:path=/run/user/1000/bus"),
            [Address::Abstract(b"/tmp/dbus-a,b".to_vec()), Address::Path(b"/run/user/1000/bus".to_vec())]
        );
        assert_eq!(addresses("unix:path=/bad%2"), []);
    }
    #[test]
    fn pipelines_auth_hello_match_and_a_non_starting_status_call() {
        let mut s = Session::start(1000);
        let (auth, sent) = sent(&s.take_output());
        assert_eq!(auth, "\0AUTH EXTERNAL 31303030\r\nBEGIN\r\n");
        let names: Vec<_> = sent.iter().map(|m| m.header().member().unwrap().to_string()).collect();
        assert_eq!(names, ["Hello", "AddMatch", "AddMatch", "GetAll"]);
        for (m, rule) in [(&sent[1], MATCH), (&sent[2], OWNERS)] {
            assert_eq!(m.body().deserialize::<&str>().unwrap(), rule);
            assert!(m.primary_header().flags().contains(zbus::message::Flags::NoReplyExpected));
        }
        let h = sent[3].header();
        assert_eq!(h.destination().unwrap().as_str(), BUS);
        assert_eq!(h.path().unwrap().as_str(), PATH);
        assert_eq!(h.interface().unwrap().as_str(), PROPERTIES);
        assert_eq!(sent[3].body().deserialize::<&str>().unwrap(), STATUS);
        assert!(sent[3].primary_header().flags().contains(zbus::message::Flags::NoAutoStart));
        assert!(sent.iter().all(|m| m.message_type() == Type::MethodCall));
        assert!(s.take_output().is_empty());
    }
    #[test]
    fn reply_decides_either_property_in_either_byte_order_fed_byte_by_byte() {
        for (is_enabled, screen_reader, endian) in
            [(false, false, Endian::Little), (true, false, Endian::Big), (false, true, Endian::Little)]
        {
            let (mut s, get_all) = started();
            let props = HashMap::from([
                ("ScreenReaderEnabled", Value::from(screen_reader)),
                ("Other", Value::from(vec!["skipped", "too"])),
                ("IsEnabled", Value::from(is_enabled)),
            ]);
            let mut input = OK.to_vec();
            input.extend(reply(&get_all, endian, &props));
            for byte in input.chunks(1) {
                assert_eq!(s.enabled(), None);
                s.feed(byte).unwrap();
            }
            assert_eq!(s.enabled(), Some(is_enabled || screen_reader));
        }
    }
    #[test]
    fn changes_from_the_owner_toggle_and_invalidation_asks_again() {
        let (mut s, get_all) = started();
        s.feed(OK).unwrap();
        s.feed(&reply(&get_all, Endian::Little, &HashMap::from([("IsEnabled", Value::from(false))]))).unwrap();
        assert_eq!(s.enabled(), Some(false));
        let on = HashMap::from([("ScreenReaderEnabled", Value::from(true))]);
        s.feed(&changed(":1.99", &on, &[])).unwrap();
        assert_eq!(s.enabled(), Some(false), "another sender is ignored");
        s.feed(&changed(OWNER, &on, &[])).unwrap();
        assert_eq!(s.enabled(), Some(true));
        s.feed(&changed(OWNER, &HashMap::from([("ScreenReaderEnabled", Value::from(false))]), &[])).unwrap();
        assert_eq!(s.enabled(), Some(false));
        assert!(s.take_output().is_empty());
        s.feed(&changed(OWNER, &HashMap::new(), &["IsEnabled"])).unwrap();
        let (_, again) = sent(&[b"BEGIN\r\n".as_slice(), &s.take_output()].concat());
        assert_eq!(again[0].header().member().unwrap().as_str(), "GetAll");
    }
    #[test]
    fn a_restarted_service_is_asked_again_and_its_signals_count() {
        let owners = |old: &str, new: &str| {
            Zmsg::signal(DBUS_PATH, DBUS, "NameOwnerChanged").unwrap().sender(DBUS).unwrap()
                .build(&(BUS, old, new)).unwrap().data().to_vec()
        };
        let (mut s, get_all) = started();
        s.feed(OK).unwrap();
        s.feed(&reply(&get_all, Endian::Little, &HashMap::from([("IsEnabled", Value::from(true))]))).unwrap();
        assert_eq!(s.enabled(), Some(true));
        s.feed(&owners(OWNER, "")).unwrap();
        assert_eq!(s.enabled(), Some(false), "the service exited");
        assert!(s.take_output().is_empty());
        s.feed(&owners("", ":1.50")).unwrap();
        let (_, again) = sent(&[b"BEGIN\r\n".as_slice(), &s.take_output()].concat());
        let mut props = HashMap::from([("IsEnabled", Value::from(false))]);
        let answer = Zmsg::method_return(&again[0].header()).unwrap().sender(":1.50").unwrap().build(&props).unwrap();
        s.feed(answer.data()).unwrap();
        assert_eq!(s.enabled(), Some(false));
        props.insert("IsEnabled", Value::from(true));
        s.feed(&changed(":1.50", &props, &[])).unwrap();
        assert_eq!(s.enabled(), Some(true), "the new owner's changes are followed");
    }
    #[test]
    fn rejected_auth_absent_service_and_garbage_close() {
        let mut s = Session::start(1000);
        assert_eq!(s.feed(b"REJECTED EXTERNAL\r\n"), Err(Gone));
        let (mut s, get_all) = started();
        s.feed(OK).unwrap();
        let error = Zmsg::error(&get_all.header(), "org.freedesktop.DBus.Error.ServiceUnknown").unwrap()
            .build(&"The name is not activatable").unwrap();
        assert_eq!(s.feed(error.data()), Err(Gone));
        let (mut s, _) = started();
        s.feed(OK).unwrap();
        assert_eq!(s.feed(&[b'x'; 16]), Err(Gone));
    }
    #[test]
    fn oversized_messages_are_read_past() {
        let (mut s, get_all) = started();
        s.feed(OK).unwrap();
        let big = Zmsg::signal("/x", "x.Y", "Z").unwrap().sender(OWNER).unwrap()
            .build(&vec![7u8; MAX_MESSAGE]).unwrap().data().to_vec();
        for chunk in big.chunks(4000) {
            s.feed(chunk).unwrap();
        }
        assert!(s.input.is_empty());
        s.feed(&reply(&get_all, Endian::Little, &HashMap::from([("IsEnabled", Value::from(true))]))).unwrap();
        assert_eq!(s.enabled(), Some(true));
    }
}
