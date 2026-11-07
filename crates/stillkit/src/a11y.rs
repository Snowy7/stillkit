//! UI Automation through AccessKit. Every surface retains its latest hit snapshot; startup
//! surfaces can defer the adapter itself until WM_GETOBJECT. Neither constructor builds a tree
//! before a client asks, and updates do no UIA work while the adapter is inactive or deferred.

use std::sync::Arc;

use accesskit::{
    Action, Node, NodeId, Role, Toggled, TreeId,
    TreeInfo, TreeUpdate,
};

use crate::scene::{Hit, HitId};

/// Values and operations that a name alone cannot describe to assistive technology.
#[derive(Debug, Clone, PartialEq)]
pub enum Semantics {
    Range {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Text(Arc<str>),
    Reorder,
    Status(Arc<str>),
    /// What a hit item says beyond its name, read-only; it can sit beside any other entry
    /// for the same hit (a range with its value as text).
    Detail(Detail),
}

/// Read-only state for [`Semantics::Detail`]; `None` leaves a property out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Detail {
    /// Help read after the name.
    pub description: Option<Arc<str>>,
    /// The value as text ("40%"), without offering to set it.
    pub value: Option<Arc<str>>,
    /// Chosen among its siblings (a tab, a list item, a segment).
    pub selected: Option<bool>,
    /// Open (a disclosure, a menu button).
    pub expanded: Option<bool>,
}

/// Rich actions remain owned data until the surface's UI thread consumes them.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessibilityAction {
    SetNumber(f64),
    SetText(String),
    Increment,
    Decrement,
    Custom(i32),
}

pub(crate) const ROOT: NodeId = NodeId(0);
pub(crate) const REORDER_BASE: u64 = 1 << 40;
const STATUS_BASE: u64 = 1 << 48;

pub(crate) fn status_node(id: HitId) -> NodeId {
    NodeId(STATUS_BASE + u64::from(id))
}

fn reorder_node(hit: HitId, direction: u64) -> NodeId {
    NodeId(REORDER_BASE + u64::from(hit) * 4 + direction)
}

pub(crate) fn node_id(hit: HitId) -> NodeId {
    NodeId(u64::from(hit) + 1)
}

/// What the tree is built from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Snapshot {
    pub role: Role,
    pub name: Arc<str>,
    pub hits: Vec<Hit>,
    pub focus: Option<HitId>,
    pub semantics: Vec<(HitId, Semantics)>,
}

impl Snapshot {
    /// AccessKit panics (and `panic = "abort"` kills the process) on a duplicate child or a
    /// focus outside the tree, so the first hit per id wins and a stale focus falls back to
    /// the root. Views should still give every hit its own id.
    pub(crate) fn tree(&self) -> TreeUpdate {
        let mut seen = std::collections::HashSet::new();
        let hits: Vec<&Hit> = self.hits.iter().filter(|h| seen.insert(h.id)).collect();
        static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if hits.len() != self.hits.len() && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            log::warn!(
                "a11y: {} hits share an id in {:?}; only the first of each is exposed",
                self.hits.len() - hits.len(),
                &*self.name
            );
        }
        let mut root = Node::new(self.role);
        root.set_label(&*self.name);
        let mut children: Vec<_> = hits.iter().map(|h| node_id(h.id)).collect();
        children.extend(self.semantics.iter().filter_map(|(id, semantic)| {
            matches!(semantic, Semantics::Status(_)).then_some(status_node(*id))
        }));
        root.set_children(children);
        let mut nodes = vec![(ROOT, root)];
        nodes.extend(hits.iter().map(|hit| {
            let mut node = Node::new(hit.role);
            node.set_label(&*hit.label);
            node.set_bounds(accesskit::Rect {
                x0: f64::from(hit.rect.left),
                y0: f64::from(hit.rect.top),
                x1: f64::from(hit.rect.right),
                y1: f64::from(hit.rect.bottom),
            });
            // A label is announced as it is; only what does something offers actions.
            if hit.actionable {
                node.add_action(Action::Click);
                node.add_action(Action::Focus);
            }
            if !hit.enabled {
                node.set_disabled();
            }
            if let Some(on) = hit.checked {
                node.set_toggled(if on { Toggled::True } else { Toggled::False });
            }
            for (_, semantic) in self.semantics.iter().filter(|(id, _)| *id == hit.id) {
                match semantic {
                    Semantics::Range {
                        value,
                        min,
                        max,
                        step,
                    } => {
                        node.set_numeric_value(*value);
                        node.set_min_numeric_value(*min);
                        node.set_max_numeric_value(*max);
                        node.set_numeric_value_step(*step);
                        if hit.actionable {
                            for action in [Action::SetValue, Action::Increment, Action::Decrement] {
                                node.add_action(action);
                            }
                        }
                    }
                    Semantics::Text(value) => {
                        node.set_value(&**value);
                        if hit.actionable {
                            node.add_action(Action::SetValue);
                        }
                    }
                    Semantics::Reorder if hit.actionable => {
                        node.add_action(Action::CustomAction);
                        node.set_custom_actions(vec![
                            accesskit::CustomAction {
                                id: 1,
                                description: "Move up".into(),
                            },
                            accesskit::CustomAction {
                                id: 2,
                                description: "Move down".into(),
                            },
                        ]);
                        // UIA has no generic custom-action pattern. Named Invoke children give
                        // Windows clients the same operations without inventing keyboard input.
                        node.set_children(vec![reorder_node(hit.id, 1), reorder_node(hit.id, 2)]);
                    }
                    Semantics::Detail(detail) => {
                        if let Some(description) = &detail.description {
                            node.set_description(&**description);
                        }
                        if let Some(value) = &detail.value {
                            node.set_value(&**value);
                        }
                        if let Some(selected) = detail.selected {
                            node.set_selected(selected);
                        }
                        if let Some(expanded) = detail.expanded {
                            node.set_expanded(expanded);
                        }
                    }
                    _ => {}
                }
            }
            (node_id(hit.id), node)
        }));
        for hit in &hits {
            if !hit.actionable
                || !self
                    .semantics
                    .iter()
                    .any(|(id, semantic)| *id == hit.id && *semantic == Semantics::Reorder)
            {
                continue;
            }
            for (direction, label) in [(1, "Move up"), (2, "Move down")] {
                let mut node = Node::new(Role::Button);
                node.set_label(label);
                node.add_action(Action::Click);
                nodes.push((reorder_node(hit.id, direction), node));
            }
        }
        nodes.extend(self.semantics.iter().filter_map(|(id, semantic)| {
            let Semantics::Status(message) = semantic else {
                return None;
            };
            let mut node = Node::new(Role::Status);
            node.set_label(&**message);
            node.set_live(accesskit::Live::Polite);
            Some((status_node(*id), node))
        }));
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(ROOT)),
            tree_id: TreeId::ROOT,
            focus: self
                .focus
                .filter(|focus| seen.contains(focus))
                .map_or(ROOT, node_id),
        }
    }
}

