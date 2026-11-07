//! VoiceOver through AccessKit's NSAccessibility adapter, fed the same tree as Windows
//! (`crate::a11y::Snapshot`). The adapter subclasses the surface's view at run time and stays
//! inactive (no tree is built) until an assistive client asks. AccessKit's coordinates are
//! physical pixels, the surface's own.
use crate::a11y::{AccessibilityAction, REORDER_BASE, Snapshot};
use crate::scene::{Hit, HitId};
use accesskit::{Action, ActionHandler, ActionRequest, ActivationHandler, Role, TreeUpdate};
use accesskit_macos::{QueuedEvents, SubclassingAdapter};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Arc};

use super::surface::{Event, SurfaceId};

type Pending = Rc<RefCell<VecDeque<(HitId, AccessibilityAction)>>>;

/// A surface's accessibility: the adapter and the latest snapshot.
pub(super) struct A11y {
    adapter: SubclassingAdapter,
    snapshot: Rc<RefCell<Rc<Snapshot>>>,
    pending: Pending,
}

impl A11y {
    /// Subclasses `view` (before anything observes it), with an empty tree named `name`.
    pub(super) fn new(view: &super::view::View, id: SurfaceId, role: Role, name: Arc<str>) -> Self {
        let snapshot = Rc::new(RefCell::new(Rc::new(Snapshot { role, name, hits: Vec::new(), focus: None, semantics: Vec::new() })));
        let pending = Pending::default();
        // SAFETY: a live view, before it is shown, focused or observed.
        let adapter = unsafe {
            SubclassingAdapter::new(
                std::ptr::from_ref(view).cast_mut().cast(),
                Initial(snapshot.clone()),
                Actions { id, pending: pending.clone() },
            )
        };
        Self { adapter, snapshot, pending }
    }

    /// New hit items, focus or semantics; the tree follows only once a client has asked for
    /// it. Raise the returned events after letting go of any borrow.
    pub(super) fn update(&mut self, hits: &[Hit], focus: Option<HitId>, semantics: &[(HitId, crate::Semantics)]) -> Option<QueuedEvents> {
        let next = {
            let old = self.snapshot.borrow();
            if old.hits == hits && old.focus == focus && old.semantics == semantics {
                return None;
            }
            Rc::new(Snapshot { hits: hits.to_vec(), focus, semantics: semantics.to_vec(), ..(**old).clone() })
        };
        *self.snapshot.borrow_mut() = next.clone();
        self.adapter.update_if_active(|| next.tree())
    }

    pub(super) fn take_action(&self, hit: HitId) -> Option<AccessibilityAction> {
        let mut pending = self.pending.borrow_mut();
        let at = pending.iter().position(|(id, _)| *id == hit)?;
        pending.remove(at).map(|(_, action)| action)
    }

    pub(super) fn focus_changed(&mut self, focused: bool) -> Option<QueuedEvents> {
        self.adapter.update_view_focus_state(focused)
    }
}

/// Builds the first tree on demand, from the latest snapshot.
struct Initial(Rc<RefCell<Rc<Snapshot>>>);

impl ActivationHandler for Initial {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.0.borrow().tree())
    }
}

/// Turns VoiceOver's requests into the surface's `Invoke` and `Focus` events (on the main
/// thread, where AccessKit calls it), keeping rich ones for `take_action`.
struct Actions {
    id: SurfaceId,
    pending: Pending,
}

impl Actions {
    fn keep(&self, hit: HitId, action: AccessibilityAction) {
        let mut pending = self.pending.borrow_mut();
        // Bound abandoned requests when a client targets a closing surface.
        if pending.len() >= 128 {
            pending.pop_front();
        }
        pending.push_back((hit, action));
    }
}

impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        if let Some(encoded) = request.target_node.0.checked_sub(REORDER_BASE) {
            if request.action != Action::Click || !matches!(encoded % 4, 1 | 2) {
                return;
            }
            let Ok(hit) = u32::try_from(encoded / 4) else { return };
            self.keep(hit, AccessibilityAction::Custom((encoded % 4) as i32));
            super::pump::push(self.id, Event::Invoke(hit));
            return;
        }
        let Some(hit) = request.target_node.0.checked_sub(1).and_then(|id| u32::try_from(id).ok()) else { return };
        let rich = match (request.action, request.data) {
            (Action::SetValue, Some(accesskit::ActionData::NumericValue(value))) => Some(AccessibilityAction::SetNumber(value)),
            (Action::SetValue, Some(accesskit::ActionData::Value(value))) => Some(AccessibilityAction::SetText(value.into())),
            (Action::Increment, _) => Some(AccessibilityAction::Increment),
            (Action::Decrement, _) => Some(AccessibilityAction::Decrement),
            (Action::CustomAction, Some(accesskit::ActionData::CustomAction(id))) => Some(AccessibilityAction::Custom(id)),
            _ => None,
        };
        if let Some(action) = rich {
            self.keep(hit, action);
        }
        let event = match request.action {
            Action::Click | Action::Expand | Action::Collapse => Event::Invoke(hit),
            Action::Focus => Event::Focus(hit),
            Action::SetValue | Action::Increment | Action::Decrement | Action::CustomAction => Event::Invoke(hit),
            _ => return,
        };
        super::pump::push(self.id, event);
    }
}
