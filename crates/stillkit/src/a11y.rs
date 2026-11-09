//! UI Automation through AccessKit. Every surface retains its latest hit snapshot; startup
//! surfaces can defer the adapter itself until WM_GETOBJECT. Neither constructor builds a tree
//! before a client asks, and updates do no UIA work while the adapter is inactive or deferred.

use std::sync::Arc;

use accesskit::{
    Action, Node, NodeId, Role, TextPosition, TextSelection, Toggled, TreeId,
    TreeInfo, TreeUpdate,
};

use crate::scene::{Hit, HitId};

/// Values and operations that a name alone cannot describe to assistive technology.
#[derive(Debug, Clone, PartialEq)]
pub enum Semantics {
    /// Range.
    Range {
        /// Current value.
        value: f64,
        /// Minimum logical window size.
        min: f64,
        /// Max.
        max: f64,
        /// Step.
        step: f64,
    },
    /// Text.
    Text(Arc<str>),
    /// Reorder.
    Reorder,
    /// Status.
    Status(Arc<str>),
    /// What a hit item says beyond its name, read-only; it can sit beside any other entry
    /// for the same hit (a range with its value as text).
    Detail(Detail),
    /// A text field's lines, characters, caret and selection (beside its [`Semantics::Text`]),
    /// as text runs assistive technology reads by character, word and line.
    Field(Arc<FieldText>),
}

/// A text field's text laid out ([`Semantics::Field`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldText {
    /// Lines.
    pub lines: Vec<TextLine>,
    /// Where the selection started, and the caret: (line, character) in `lines`.
    pub anchor: (usize, usize),
    /// Caret.
    pub caret: (usize, usize),
}

/// One visual line of a field ([`FieldText`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextLine {
    /// Text content.
    pub text: Arc<str>,
    /// Where it is, in the surface's pixels.
    pub rect: crate::Rect,
    /// Where each character starts and how wide it is, px from the line's left.
    pub chars: Vec<(f32, f32)>,
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
    /// What it opens (a menu button, a dialog's trigger), open or not.
    pub popup: Option<accesskit::HasPopup>,
    /// The hit item it opened and controls (the menu or popover panel), while open.
    pub controls: Option<HitId>,
    /// A modal dialog: everything outside it is inert.
    pub modal: bool,
    /// Its value fails validation (a field with an error).
    pub invalid: bool,
    /// The hit item it shows as active while it keeps focus itself (a combobox's highlighted
    /// row), read as if it had focus.
    pub active_descendant: Option<HitId>,
    /// Virtual collection counts, indexes and tree positions, allocated only when used.
    pub collection: Option<Box<Collection>>,
}

/// Counts and positions in a virtual grid or tree; indexes are zero-based, tree positions one-based.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Collection {
    /// Total rows, including virtual rows.
    pub row_count: Option<usize>,
    /// Total columns.
    pub column_count: Option<usize>,
    /// Zero-based row index in the complete model.
    pub row_index: Option<usize>,
    /// Zero-based column index.
    pub column_index: Option<usize>,
    /// One-based tree depth.
    pub level: Option<usize>,
    /// Number of siblings.
    pub size_of_set: Option<usize>,
    /// One-based sibling position.
    pub position_in_set: Option<usize>,
}

/// Rich actions remain owned data until the surface's UI thread consumes them.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessibilityAction {
    /// Set number.
    SetNumber(f64),
    /// Set text.
    SetText(String),
    /// Increment.
    Increment,
    /// Decrement.
    Decrement,
    /// Custom.
    Custom(i32),
}

pub(crate) const ROOT: NodeId = NodeId(0);
pub(crate) const REORDER_BASE: u64 = 1 << 40;
const STATUS_BASE: u64 = 1 << 48;
/// Text runs: a field's lines, 4096 a field at most.
const RUN_BASE: u64 = 1 << 44;

fn run_node(hit: HitId, line: usize) -> NodeId {
    NodeId(RUN_BASE + u64::from(hit) * 4096 + line.min(4095) as u64)
}

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
            // A status item (a toast) is announced when it appears.
            if hit.role == Role::Status {
                node.set_live(accesskit::Live::Polite);
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
                    Semantics::Field(text) => {
                        let n = text.lines.len().min(4096);
                        node.set_children((0..n).map(|line| run_node(hit.id, line)).collect::<Vec<_>>());
                        let at = |(line, ch): (usize, usize)| TextPosition {
                            node: run_node(hit.id, line.min(n.saturating_sub(1))),
                            character_index: ch,
                        };
                        if n > 0 {
                            node.set_text_selection(TextSelection { anchor: at(text.anchor), focus: at(text.caret) });
                        }
                    }
                    Semantics::Detail(detail) => {
                        if let Some(collection) = &detail.collection {
                            if let Some(value) = collection.row_count { node.set_row_count(value); }
                            if let Some(value) = collection.column_count { node.set_column_count(value); }
                            if let Some(value) = collection.row_index { node.set_row_index(value); }
                            if let Some(value) = collection.column_index { node.set_column_index(value); }
                            if let Some(value) = collection.level { node.set_level(value); }
                            if let Some(value) = collection.size_of_set { node.set_size_of_set(value); }
                            if let Some(value) = collection.position_in_set { node.set_position_in_set(value); }
                        }
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
                            // A trigger opens and closes with its press; offer only the one
                            // that changes something, so a client's Expand never closes it.
                            node.add_action(if expanded { Action::Collapse } else { Action::Expand });
                        }
                        if let Some(popup) = detail.popup {
                            node.set_has_popup(popup);
                        }
                        // A relation to an item outside this surface's tree is left out.
                        if let Some(controls) = detail.controls.filter(|c| seen.contains(c)) {
                            node.push_controlled(node_id(controls));
                        }
                        if detail.modal {
                            node.set_modal();
                        }
                        if detail.invalid {
                            node.set_invalid(accesskit::Invalid::True);
                        }
                        if let Some(active) = detail.active_descendant.filter(|a| seen.contains(a)) {
                            node.set_active_descendant(node_id(active));
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
        for (id, semantic) in &self.semantics {
            let Semantics::Field(text) = semantic else { continue };
            if !seen.contains(id) {
                continue;
            }
            for (line, l) in text.lines.iter().take(4096).enumerate() {
                nodes.push((run_node(*id, line), text_run(l)));
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


/// One line of a field as an AccessKit text run: its characters' UTF-8 lengths, places and
/// widths, and where its words start (the first 256 characters).
fn text_run(l: &TextLine) -> Node {
    use unicode_segmentation::UnicodeSegmentation;
    let mut node = Node::new(Role::TextRun);
    node.set_value(&*l.text);
    node.set_bounds(accesskit::Rect {
        x0: f64::from(l.rect.left),
        y0: f64::from(l.rect.top),
        x1: f64::from(l.rect.right),
        y1: f64::from(l.rect.bottom),
    });
    let lengths: Vec<u8> = l.text.chars().map(|c| c.len_utf8() as u8).collect();
    let n = lengths.len();
    node.set_character_lengths(lengths);
    let place = |k: usize| l.chars.get(k).copied().unwrap_or_default();
    node.set_character_positions((0..n).map(|k| place(k).0).collect::<Vec<_>>());
    node.set_character_widths((0..n).map(|k| place(k).1).collect::<Vec<_>>());
    // Words start where Ctrl+Right lands: at each word, its trailing spaces part of it.
    let mut starts = Vec::new();
    let mut at = 0usize;
    for word in l.text.split_word_bounds() {
        if !word.chars().all(char::is_whitespace) && at <= usize::from(u8::MAX) {
            starts.push(at as u8);
        }
        at += word.chars().count();
    }
    node.set_word_starts(starts);
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    /// A field's lines become text runs under it, with its selection on them.
    #[test]
    fn a_field_reads_as_text_runs_with_its_selection() {
        let hit = Hit {
            id: 7,
            rect: Rect::new(0.0, 0.0, 100.0, 40.0),
            role: Role::MultilineTextInput,
            label: "Notes".into(),
            enabled: true,
            actionable: true,
            checked: None,
        };
        let line = |text: &str, top: f32| TextLine {
            text: text.into(),
            rect: Rect::new(0.0, top, 50.0, 20.0),
            chars: (0..text.chars().count()).map(|k| (k as f32 * 8.0, 8.0)).collect(),
        };
        let text = FieldText { lines: vec![line("one two ", 0.0), line("three", 20.0)], anchor: (0, 4), caret: (1, 2) };
        let tree = Snapshot {
            role: Role::Window,
            name: "w".into(),
            hits: vec![hit],
            focus: Some(7),
            semantics: vec![(7, Semantics::Text("one two three".into())), (7, Semantics::Field(Arc::new(text)))],
        }
        .tree();
        let field = &tree.nodes.iter().find(|(id, _)| *id == node_id(7)).unwrap().1;
        assert_eq!(field.children(), [run_node(7, 0), run_node(7, 1)]);
        let sel = field.text_selection().unwrap();
        assert_eq!((sel.anchor.node, sel.anchor.character_index), (run_node(7, 0), 4));
        assert_eq!((sel.focus.node, sel.focus.character_index), (run_node(7, 1), 2));
        let run = &tree.nodes.iter().find(|(id, _)| *id == run_node(7, 0)).unwrap().1;
        assert_eq!(run.role(), Role::TextRun);
        assert_eq!(run.value(), Some("one two "));
        assert_eq!(run.word_starts(), [0, 4]);
        assert_eq!(run.character_positions().map(<[f32]>::len), Some(8));
    }
}
