//! AT-SPI through AccessKit Unix, on by default but created only while assistive technology is
//! on. `Gate` decides per thread: `STILLKIT_ACCESSIBILITY` (`1` on, `0` off) or, unset, the
//! `org.a11y.Status` properties watched on one session-bus socket in the Wayland poll
//! (`a11y_bus.rs`). Each window's `A11y` keeps its tree current without an adapter and brings
//! one up, with that tree, when the gate opens; it drops it when the gate closes.
use super::a11y_bus::{Address, Gone, Mode, Session};
use super::surface::Event;
use crate::{AccessibilityAction, HitId, a11y::Snapshot};
use accesskit::{
    Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Role, TreeUpdate,
};
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{Arc, Mutex},
};

/// Whether this thread's windows should have AT-SPI adapters.
pub(super) struct Gate {
    enabled: bool,
    bus: Option<(UnixStream, Session)>,
}
impl Gate {
    /// Reads the override; in automatic mode opens the status socket and sends the whole
    /// conversation in one non-blocking write. No session bus: no socket, never enabled.
    pub(super) fn new() -> Self {
        let mode = Mode::from_env(std::env::var("STILLKIT_ACCESSIBILITY").ok().as_deref());
        let bus = if mode == Mode::Auto { connect() } else { None };
        Self { enabled: mode == Mode::On, bus }
    }
    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }
    /// The status socket for the poll set, or -1 (which poll skips).
    pub(super) fn fd(&self) -> RawFd {
        self.bus.as_ref().map_or(-1, |(stream, _)| stream.as_raw_fd())
    }
    /// Handles a readable (or hung-up) status socket; returns the decision when it changed.
    /// On error, EOF or an absent status service the socket is closed and leaves the poll, and
    /// the last answer stands (off if none came).
    pub(super) fn ready(&mut self) -> Option<bool> {
        let (stream, session) = self.bus.as_mut()?;
        let result = exchange(stream, session);
        let answer = session.enabled();
        if result.is_err() {
            log::debug!("AT-SPI status: session bus closed or org.a11y.Bus absent");
            self.bus = None;
        }
        let enabled = answer.unwrap_or(self.enabled);
        (enabled != self.enabled).then(|| {
            self.enabled = enabled;
            enabled
        })
    }
}
/// Reads until the socket would block, then writes anything the session queued.
fn exchange(stream: &mut UnixStream, session: &mut Session) -> Result<(), Gone> {
    let mut bytes = [0; 4096];
    loop {
        match stream.read(&mut bytes) {
            Ok(0) => return Err(Gone),
            Ok(n) => session.feed(&bytes[..n])?,
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(Gone),
        }
    }
    send(stream, &session.take_output())
}
/// Writes a whole message batch at once; a short write (never seen for these few hundred
/// bytes on a fresh socket) is treated as a broken connection rather than buffered.
fn send(stream: &mut UnixStream, bytes: &[u8]) -> Result<(), Gone> {
    if bytes.is_empty() || stream.write(bytes).is_ok_and(|n| n == bytes.len()) { Ok(()) } else { Err(Gone) }
}
fn connect() -> Option<(UnixStream, Session)> {
    let list = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
    let addresses = match &list {
        Some(list) => super::a11y_bus::addresses(list),
        None => {
            let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
            vec![Address::Path([std::os::unix::ffi::OsStrExt::as_bytes(dir.as_os_str()), b"/bus"].concat())]
        }
    };
    let mut stream = addresses.iter().find_map(open)?;
    // SAFETY: getuid has no preconditions and cannot fail.
    let mut session = Session::start(unsafe { libc::getuid() });
    send(&mut stream, &session.take_output()).ok()?;
    Some((stream, session))
}
/// A non-blocking connect: a local socket either accepts at once or is refused (a full backlog
/// returns EAGAIN), so startup never waits on the bus.
fn open(address: &Address) -> Option<UnixStream> {
    let (prefix, name): (&[u8], &[u8]) = match address {
        Address::Path(path) => (b"", path),
        Address::Abstract(name) => (b"\0", name),
    };
    // SAFETY: sockaddr_un is plain data; all-zero is a valid value.
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as _;
    let path = [prefix, name].concat();
    // Pathnames keep a trailing NUL inside sun_path; abstract names are counted exactly.
    if path.len() + usize::from(prefix.is_empty()) > addr.sun_path.len() {
        return None;
    }
    for (slot, byte) in addr.sun_path.iter_mut().zip(&path) {
        *slot = *byte as _;
    }
    let len = std::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + usize::from(prefix.is_empty());
    // SAFETY: plain socket creation; the result is checked before use.
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return None;
    }
    // SAFETY: fd is a new socket we own.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: addr is an initialized sockaddr_un and len is within its size.
    let connected = unsafe { libc::connect(fd.as_raw_fd(), (&raw const addr).cast(), len as libc::socklen_t) } == 0;
    connected.then(|| UnixStream::from(fd))
}

type Requests = Arc<Mutex<VecDeque<(Event, Option<(HitId, AccessibilityAction)>)>>>;
/// One window's accessibility: the retained tree, and the adapter while the gate is open.
pub(super) struct A11y {
    live: Option<Live>,
    snapshot: Arc<Mutex<Snapshot>>,
    requests: Requests,
    pending: VecDeque<(HitId, AccessibilityAction)>,
    focused: bool,
}
/// An adapter and the socket its action handler writes to, waking the poll.
struct Live {
    adapter: accesskit_unix::Adapter,
    reader: UnixStream,
}
impl A11y {
    pub(super) fn new(role: Role, name: &str, enabled: bool) -> Self {
        let snapshot = Arc::new(Mutex::new(Snapshot { role, name: name.into(), hits: vec![], focus: None, semantics: vec![] }));
        let mut result = Self { live: None, snapshot, requests: Requests::default(), pending: VecDeque::new(), focused: false };
        result.set_enabled(enabled);
        result
    }
    /// Brings the adapter up (AccessKit asks for the latest tree when a client activates it),
    /// or drops it. AccessKit Unix keeps its executor thread and session-bus connection for
    /// the life of the process once the first adapter exists; see docs/ui.md, "Linux accessibility".
    pub(super) fn set_enabled(&mut self, enabled: bool) {
        if enabled == self.live.is_some() {
            return;
        }
        if !enabled {
            self.live = None;
            return;
        }
        let pair = UnixStream::pair().and_then(|(reader, writer)| {
            reader.set_nonblocking(true)?;
            writer.set_nonblocking(true)?;
            Ok((reader, writer))
        });
        match pair {
            Ok((reader, writer)) => {
                let mut adapter = accesskit_unix::Adapter::new(
                    Initial(self.snapshot.clone()),
                    Actions { requests: self.requests.clone(), writer },
                    Inactive,
                );
                if self.focused {
                    adapter.update_window_focus_state(true);
                }
                self.live = Some(Live { adapter, reader });
            }
            Err(e) => log::error!("AT-SPI wake descriptor: {e}"),
        }
    }
    pub(super) fn update(&mut self, hits: &[crate::Hit], focus: Option<HitId>) {
        let mut snapshot = self.snapshot.lock().unwrap();
        if snapshot.hits == hits && snapshot.focus == focus { return; }
        snapshot.hits = hits.to_vec();
        snapshot.focus = focus;
        // Release the snapshot lock before taking AccessKit's state lock: activation runs
        // under that lock on its executor and reads the same snapshot.
        drop(snapshot);
        self.publish();
    }
    fn publish(&mut self) {
        if let Some(live) = &mut self.live {
            live.adapter.update_if_active(|| self.snapshot.lock().unwrap().tree());
        }
    }
    pub(super) fn semantics(&mut self, semantics: Vec<(HitId, crate::Semantics)>) {
        let mut snapshot = self.snapshot.lock().unwrap();
        if snapshot.semantics == semantics { return; }
        snapshot.semantics = semantics;
        drop(snapshot);
        self.publish();
    }
    pub(super) fn focus(&mut self, focused: bool) {
        self.focused = focused;
        if let Some(live) = &mut self.live { live.adapter.update_window_focus_state(focused); }
    }
    pub(super) fn fd(&self) -> RawFd { self.live.as_ref().map_or(-1, |live| live.reader.as_raw_fd()) }
    pub(super) fn drain(&mut self) -> Vec<Event> {
        if let Some(live) = &mut self.live {
            let mut bytes = [0; 128];
            while live.reader.read(&mut bytes).is_ok_and(|n| n > 0) {}
        }
        let mut events = Vec::new();
        for (event, rich) in self.requests.lock().unwrap().drain(..) {
            if let Some(rich) = rich {
                if self.pending.len() >= 128 { self.pending.pop_front(); }
                self.pending.push_back(rich);
            }
            events.push(event);
        }
        events
    }
    pub(super) fn take(&mut self, hit: HitId) -> Option<AccessibilityAction> {
        let at = self.pending.iter().position(|(id, _)| *id == hit)?;
        self.pending.remove(at).map(|(_, action)| action)
    }
}
struct Initial(Arc<Mutex<Snapshot>>);
impl ActivationHandler for Initial {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> { Some(self.0.lock().unwrap().tree()) }
}
struct Inactive;
impl DeactivationHandler for Inactive { fn deactivate_accessibility(&mut self) {} }
struct Actions { requests: Requests, writer: UnixStream }
impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        let (hit, rich) = if let Some(encoded) = request.target_node.0.checked_sub(crate::a11y::REORDER_BASE) {
            if request.action != Action::Click || !matches!(encoded % 4, 1 | 2) { return; }
            let Ok(hit) = u32::try_from(encoded / 4) else { return };
            (hit, Some(AccessibilityAction::Custom((encoded % 4) as i32)))
        } else {
            let Some(hit) = request.target_node.0.checked_sub(1).and_then(|id| u32::try_from(id).ok()) else { return };
            let rich = match (request.action, request.data) {
                (Action::SetValue, Some(accesskit::ActionData::NumericValue(value))) => Some(AccessibilityAction::SetNumber(value)),
                (Action::SetValue, Some(accesskit::ActionData::Value(value))) => Some(AccessibilityAction::SetText(value.into())),
                (Action::Increment, _) => Some(AccessibilityAction::Increment),
                (Action::Decrement, _) => Some(AccessibilityAction::Decrement),
                (Action::CustomAction, Some(accesskit::ActionData::CustomAction(id))) => Some(AccessibilityAction::Custom(id)),
                _ => None,
            };
            (hit, rich)
        };
        let event = match request.action {
            Action::Focus => Event::Focus(hit),
            Action::Click | Action::Expand | Action::Collapse | Action::SetValue | Action::Increment | Action::Decrement | Action::CustomAction => Event::Invoke(hit),
            _ => return,
        };
        let mut requests = self.requests.lock().unwrap();
        if requests.len() >= 128 { requests.pop_front(); }
        requests.push_back((event, rich.map(|action| (hit, action))));
        drop(requests);
        let _ = self.writer.write(&[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rich_requests_wake_and_preserve_action_payloads() {
        let (mut reader, writer) = UnixStream::pair().unwrap();
        reader.set_nonblocking(true).unwrap();
        let requests = Requests::default();
        let mut actions = Actions { requests: requests.clone(), writer };
        actions.do_action(ActionRequest {
            action: Action::SetValue,
            target_tree: accesskit::TreeId::ROOT,
            target_node: crate::a11y::node_id(7),
            data: Some(accesskit::ActionData::NumericValue(42.0)),
        });
        let mut byte = [0];
        assert_eq!(reader.read(&mut byte).unwrap(), 1);
        assert_eq!(requests.lock().unwrap().pop_front(), Some((Event::Invoke(7), Some((7, AccessibilityAction::SetNumber(42.0))))));
    }
    #[test]
    fn activation_reads_the_latest_snapshot() {
        let snapshot = Arc::new(Mutex::new(Snapshot { role: Role::Window, name: "first".into(), hits: vec![], focus: None, semantics: vec![] }));
        let mut initial = Initial(snapshot.clone());
        snapshot.lock().unwrap().name = "latest".into();
        let tree = initial.request_initial_tree().unwrap();
        assert_eq!(tree.nodes[0].1.label(), Some("latest"));
    }
}
