//! Opt-in AT-SPI bridge. No executor, D-Bus connection or wake descriptor unless enabled.
use crate::{a11y::Snapshot, AccessibilityAction, HitId};
use super::surface::Event;
use accesskit::{Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Role, TreeUpdate};
use std::{collections::VecDeque, io::{Read, Write}, os::{fd::{AsRawFd, RawFd}, unix::net::UnixStream}, sync::{Arc, Mutex}};

type Requests = Arc<Mutex<VecDeque<(Event, Option<(HitId, AccessibilityAction)>)>>>;
pub(super) struct A11y {
    adapter: Option<accesskit_unix::Adapter>,
    snapshot: Arc<Mutex<Snapshot>>,
    requests: Requests,
    pending: VecDeque<(HitId, AccessibilityAction)>,
    reader: Option<UnixStream>,
}
impl A11y {
    pub(super) fn new(role: Role, name: &str) -> Self {
        let snapshot = Arc::new(Mutex::new(Snapshot { role, name: name.into(), hits: vec![], focus: None, semantics: vec![] }));
        let requests = Requests::default();
        let mut result = Self { adapter: None, snapshot, requests, pending: VecDeque::new(), reader: None };
        if std::env::var("STILLKIT_ACCESSIBILITY").as_deref() == Ok("1") {
            match UnixStream::pair().and_then(|(reader, writer)| {
                reader.set_nonblocking(true)?;
                writer.set_nonblocking(true)?;
                Ok((reader, writer))
            }) {
                Ok((reader, writer)) => {
                    result.reader = Some(reader);
                    result.adapter = Some(accesskit_unix::Adapter::new(
                        Initial(result.snapshot.clone()),
                        Actions { requests: result.requests.clone(), writer },
                        Inactive,
                    ));
                }
                Err(e) => log::error!("AT-SPI wake descriptor: {e}"),
            }
        }
        result
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
        if let Some(adapter) = &mut self.adapter {
            adapter.update_if_active(|| self.snapshot.lock().unwrap().tree());
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
        if let Some(adapter) = &mut self.adapter { adapter.update_window_focus_state(focused); }
    }
    pub(super) fn fd(&self) -> RawFd { self.reader.as_ref().map_or(-1, AsRawFd::as_raw_fd) }
    pub(super) fn drain(&mut self) -> Vec<Event> {
        if let Some(reader) = &mut self.reader {
            let mut bytes = [0; 128];
            while reader.read(&mut bytes).is_ok_and(|n| n > 0) {}
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
