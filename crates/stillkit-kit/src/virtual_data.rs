//! Model-owned virtual data controls. Rendering touches only the built rows.
use super::*;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use stillkit::el::{Handled, KeyCode, Modifiers, PointerPhase, VirtualRows, variable_list};

/// Selection shared by data controls; keys survive sorting and filtering.
#[derive(Default)]
pub struct DataSelection {
    /// Allows Ctrl toggles and Shift ranges.
    pub multi: bool,
    /// Selected stable keys.
    pub selected: HashSet<Arc<str>>,
    /// Row used by keyboard navigation.
    pub cursor: Option<Arc<str>>,
    anchor: Option<Arc<str>>,
    typed: String,
    typed_at: Option<Instant>,
}
impl DataSelection {
    /// Selects a key with desktop Ctrl/Shift conventions in the supplied visible order.
    pub fn click(&mut self, keys: &[Arc<str>], index: usize, mods: Modifiers) {
        let Some(key) = keys.get(index) else { return };
        if self.multi && mods.shift {
            let a = self
                .anchor
                .as_ref()
                .and_then(|k| keys.iter().position(|v| v == k))
                .unwrap_or(index);
            if !mods.ctrl && !mods.meta {
                self.selected.clear();
            }
            self.selected
                .extend(keys[a.min(index)..=a.max(index)].iter().cloned());
        } else {
            if self.multi && (mods.ctrl || mods.meta) {
                if !self.selected.remove(key) {
                    self.selected.insert(key.clone());
                }
            } else {
                self.selected.clear();
                self.selected.insert(key.clone());
            }
            self.anchor = Some(key.clone());
        }
        self.cursor = Some(key.clone());
    }
    fn navigate(&mut self, keys: &[Arc<str>], key: KeyCode, mods: Modifiers, page: usize) -> bool {
        if keys.is_empty() {
            return false;
        }
        let at = self
            .cursor
            .as_ref()
            .and_then(|k| keys.iter().position(|v| v == k));
        let last = keys.len() - 1;
        let next = match key {
            KeyCode::UP => at.map_or(last, |i| i.saturating_sub(1)),
            KeyCode::DOWN => at.map_or(0, |i| (i + 1).min(last)),
            KeyCode::HOME => 0,
            KeyCode::END => last,
            KeyCode::PAGE_UP => at.unwrap_or(0).saturating_sub(page),
            KeyCode::PAGE_DOWN => (at.unwrap_or(0) + page).min(last),
            _ => return false,
        };
        self.click(keys, next, mods);
        true
    }
    fn type_ahead(&mut self, keys: &[Arc<str>], labels: &[Arc<str>], c: char) -> bool {
        if c.is_control() || keys.is_empty() {
            return false;
        }
        let now = Instant::now();
        if self
            .typed_at
            .is_none_or(|t| now.duration_since(t) > Duration::from_millis(700))
        {
            self.typed.clear();
        }
        self.typed_at = Some(now);
        self.typed.extend(c.to_lowercase());
        let cycle = self.typed.chars().all(|v| self.typed.starts_with(v));
        let prefix = if cycle {
            c.to_lowercase().to_string()
        } else {
            self.typed.clone()
        };
        let at = self
            .cursor
            .as_ref()
            .and_then(|k| keys.iter().position(|v| v == k));
        let start = at.map_or(0, |i| if cycle { i + 1 } else { i });
        if let Some(i) = (0..keys.len())
            .map(|i| (start + i) % keys.len())
            .find(|&i| labels[i].to_lowercase().starts_with(&prefix))
        {
            self.click(keys, i, Modifiers::NONE);
        }
        true
    }
}
/// Persistent width and bounds for a resizable table column.
#[derive(Clone)]
pub struct TableColumn {
    /// Header label.
    pub label: Arc<str>,
    /// Current width in design pixels.
    pub width: f32,
    /// Minimum width.
    pub min: f32,
    /// Maximum width.
    pub max: f32,
}
impl TableColumn {
    /// Creates a column with a clamped starting width.
    pub fn new(label: impl Into<Arc<str>>, width: f32, min: f32, max: f32) -> Self {
        let min = min.max(1.);
        let max = max.max(min);
        Self {
            label: label.into(),
            width: width.clamp(min, max),
            min,
            max,
        }
    }
}
/// Table state retained by the application; constructing the view never clones the dataset.
pub struct TableModel {
    /// Persistent columns; serialize these widths in the application if wanted.
    pub columns: Vec<TableColumn>,
    /// Keyed selection and keyboard cursor.
    pub selection: DataSelection,
    /// Requested sort order. The app applies its own data comparator.
    pub sort: Option<(usize, SortDirection)>,
    keys: Vec<Arc<str>>,
    labels: Vec<Arc<str>>,
    rows: VirtualRows,
}
impl TableModel {
    /// Creates an empty table. Populate its order with `set_rows`.
    pub fn new(columns: Vec<TableColumn>) -> Self {
        Self {
            columns,
            selection: DataSelection::default(),
            sort: None,
            keys: Vec::new(),
            labels: Vec::new(),
            rows: VirtualRows::new(28.),
        }
    }
    /// Replaces the order and type-ahead labels after data changes or sorting.
    pub fn set_rows(&mut self, rows: impl IntoIterator<Item = (Arc<str>, Arc<str>)>) {
        (self.keys, self.labels) = rows.into_iter().unzip();
        self.rows.set_keys(self.keys.iter().cloned());
    }
    /// Sorts row labels while retaining keys, measurements and selection.
    pub fn sort_by(&mut self, mut compare: impl FnMut(&str, &str) -> std::cmp::Ordering) {
        let mut order: Vec<_> = self
            .keys
            .iter()
            .cloned()
            .zip(self.labels.iter().cloned())
            .collect();
        order.sort_by(|a, b| compare(&a.1, &b.1));
        self.set_rows(order);
    }
    /// The stable key at a visible index, for custom cell rendering.
    pub fn key(&self, index: usize) -> Option<&str> {
        self.keys.get(index).map(AsRef::as_ref)
    }
    /// Reveals an unbuilt row on the next app update.
    pub fn reveal(&self, key: impl Into<Arc<str>>) {
        self.rows.reveal(key.into());
    }
    fn reveal_cursor(&self) {
        if let Some(key) = &self.selection.cursor {
            self.rows.reveal(key.clone());
        }
    }
}
impl<S: 'static> Table<S> {
    /// Builds only visible rows with custom cells, a sticky header, bounded column drags,
    /// desktop selection and keyboard navigation. `model` accesses app-owned state in handlers.
    #[allow(clippy::too_many_arguments)]
    pub fn virtual_rows(
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        state: &TableModel,
        model: fn(&mut S) -> &mut TableModel,
        height: f32,
        cell: impl Fn(usize, usize) -> El<S>,
        open: impl Fn(&mut S, Arc<str>) + 'static,
        sort: impl Fn(&mut S, (usize, SortDirection)) + 'static,
    ) -> El<S> {
        let key = key.into();
        let sort = Arc::new(sort);
        let mut header = row().h(28.).role(Role::Row).row_index(0).fill(token::CARD);
        for (i, column) in state.columns.iter().enumerate() {
            let next = if state.sort == Some((i, SortDirection::Ascending)) {
                SortDirection::Descending
            } else {
                SortDirection::Ascending
            };
            let sort = sort.clone();
            let title = if state.sort.is_some_and(|(c, _)| c == i) {
                format!(
                    "{} {}",
                    column.label,
                    if next == SortDirection::Descending {
                        "\u{2191}"
                    } else {
                        "\u{2193}"
                    }
                )
            } else {
                column.label.to_string()
            };
            let handle = el()
                .w(6.)
                .h(28.)
                .cursor(stillkit::Cursor::ResizeHorizontal)
                .key(format!("{key}/resize/{i}"))
                .on_pointer(move |s, ev, _| {
                    if ev.phase == PointerPhase::Move {
                        let m = model(s);
                        let c = &mut m.columns[i];
                        c.width = (c.width + ev.delta.x).clamp(c.min, c.max);
                        true
                    } else {
                        false
                    }
                });
            header = header.child(
                row()
                    .w(column.width)
                    .h(28.)
                    .child(
                        text(title)
                            .grow(1.)
                            .role(Role::ColumnHeader)
                            .column_index(i)
                            .key(format!("{key}/sort/{i}"))
                            .on_press(move |s| {
                                model(s).sort = Some((i, next));
                                sort(s, (i, next));
                            }),
                    )
                    .child(handle),
            );
        }
        let body = variable_list(format!("{key}/body"), &state.rows, |r| {
            let id = state.keys[r].clone();
            let click_id = id.clone();
            let focus_key = key.clone();
            let selected = state.selection.selected.contains(&id);
            let mut line = row()
                .h(28.)
                .role(Role::Row)
                .row_index(r + 1)
                .label(state.labels[r].clone())
                .selected(selected)
                .fill(if selected {
                    Paint::Theme(|t| t.color.accent_wash)
                } else {
                    Paint::Rgba(Rgba::TRANSPARENT)
                });
            for (c, column) in state.columns.iter().enumerate() {
                line = line.child(
                    el().w(column.width)
                        .h(28.)
                        .role(Role::Cell)
                        .row_index(r + 1)
                        .column_index(c)
                        .child(cell(r, c)),
                );
            }
            line.on_press_with(move |s, tx| {
                let m = model(s);
                let i = m.keys.iter().position(|k| k == &click_id).unwrap_or(r);
                m.selection.click(&m.keys, i, tx.modifiers());
                tx.focus(focus_key.clone());
            })
        })
        .h(height)
        .role(Role::RowGroup);
        let page = (height / 28.).floor().max(1.) as usize;
        let mut root = col()
            .key(key)
            .role(Role::Grid)
            .label(label)
            .row_count(state.keys.len() + 1)
            .column_count(state.columns.len())
            .focusable()
            .child(header)
            .child(body);
        if let Some(cursor) = &state.selection.cursor {
            root = root.active_descendant(cursor.clone());
        }
        root.on_key(move |s, k, tx| {
            let m = model(s);
            if k == KeyCode::ENTER {
                let id = m.selection.cursor.clone();
                if let Some(id) = id {
                    open(s, id);
                }
                return Handled::Yes;
            }
            let used = m.selection.navigate(&m.keys, k, tx.modifiers(), page);
            if used {
                m.reveal_cursor();
            }
            used.into()
        })
        .on_char(move |s, c, _| {
            let m = model(s);
            let used = m.selection.type_ahead(&m.keys, &m.labels, c);
            if used {
                m.reveal_cursor();
            }
            used.into()
        })
    }
}

struct ModelNode {
    key: Arc<str>,
    label: Arc<str>,
    parent: Option<usize>,
    children: Vec<usize>,
    expanded: bool,
    level: usize,
    position: usize,
    set_size: usize,
}
/// Cached flattened tree with keyed selection. Rebuilds visibility only on model edits.
pub struct TreeModel {
    nodes: Vec<ModelNode>,
    roots: Vec<usize>,
    visible: Vec<usize>,
    keys: Vec<Arc<str>>,
    labels: Vec<Arc<str>>,
    shown: Option<HashSet<usize>>,
    rows: VirtualRows,
    /// Selection in visible tree order.
    pub selection: DataSelection,
}
impl TreeModel {
    /// Takes a forest with stable, globally unique node keys.
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        fn append(
            nodes: Vec<TreeNode>,
            parent: Option<usize>,
            level: usize,
            out: &mut Vec<ModelNode>,
            selected: &mut HashSet<Arc<str>>,
        ) -> Vec<usize> {
            nodes
                .into_iter()
                .map(|n| {
                    let i = out.len();
                    if n.selected {
                        selected.insert(n.key.clone());
                    }
                    out.push(ModelNode {
                        key: n.key,
                        label: n.label,
                        parent,
                        children: Vec::new(),
                        expanded: n.expanded,
                        level,
                        position: 0,
                        set_size: 0,
                    });
                    let children = append(n.children, Some(i), level + 1, out, selected);
                    out[i].children = children;
                    i
                })
                .collect()
        }
        let mut m = Self {
            nodes: Vec::new(),
            roots: Vec::new(),
            visible: Vec::new(),
            keys: Vec::new(),
            labels: Vec::new(),
            shown: None,
            rows: VirtualRows::new(28.),
            selection: DataSelection::default(),
        };
        m.roots = append(nodes, None, 1, &mut m.nodes, &mut m.selection.selected);
        m.rebuild();
        m
    }
    fn rebuild(&mut self) {
        let sets = std::iter::once(self.roots.clone())
            .chain(self.nodes.iter().map(|n| n.children.clone()))
            .collect::<Vec<_>>();
        for set in sets {
            let shown: Vec<_> = set
                .into_iter()
                .filter(|i| self.shown.as_ref().is_none_or(|s| s.contains(i)))
                .collect();
            for (pos, &i) in shown.iter().enumerate() {
                self.nodes[i].position = pos + 1;
                self.nodes[i].set_size = shown.len();
            }
        }
        self.visible.clear();
        let mut stack: Vec<_> = self.roots.iter().rev().copied().collect();
        while let Some(i) = stack.pop() {
            if self.shown.as_ref().is_some_and(|s| !s.contains(&i)) {
                continue;
            }
            self.visible.push(i);
            if self.nodes[i].expanded {
                stack.extend(self.nodes[i].children.iter().rev().copied());
            }
        }
        self.keys = self
            .visible
            .iter()
            .map(|&i| self.nodes[i].key.clone())
            .collect();
        self.labels = self
            .visible
            .iter()
            .map(|&i| self.nodes[i].label.clone())
            .collect();
        self.rows.set_keys(self.keys.iter().cloned());
    }
    /// Number of visible rows, including ancestors kept by a filter.
    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }
    /// Key at a visible row.
    pub fn visible_key(&self, row: usize) -> Option<&str> {
        self.keys.get(row).map(AsRef::as_ref)
    }
    /// Expands or collapses a branch. A cursor inside a closing branch moves to it.
    pub fn set_expanded(&mut self, key: &str, expanded: bool) {
        let Some(i) = self.nodes.iter().position(|n| n.key.as_ref() == key) else {
            return;
        };
        self.nodes[i].expanded = expanded;
        if !expanded && let Some(cursor) = &self.selection.cursor {
            let mut at = self.nodes.iter().position(|n| &n.key == cursor);
            while let Some(j) = at {
                if self.nodes[j].parent == Some(i) {
                    self.selection.cursor = Some(self.nodes[i].key.clone());
                    break;
                }
                at = self.nodes[j].parent;
            }
        }
        self.rebuild();
    }
    /// Opens every ancestor, selects the node and reveals it even under a filter.
    pub fn reveal(&mut self, key: &str) -> bool {
        let Some(i) = self.nodes.iter().position(|n| n.key.as_ref() == key) else {
            return false;
        };
        let mut at = Some(i);
        while let Some(j) = at {
            if let Some(shown) = &mut self.shown {
                shown.insert(j);
            }
            if j != i {
                self.nodes[j].expanded = true;
            }
            at = self.nodes[j].parent;
        }
        self.rebuild();
        if let Some(row) = self.keys.iter().position(|k| k.as_ref() == key) {
            self.selection.click(&self.keys, row, Modifiers::NONE);
        }
        self.rows.reveal(self.nodes[i].key.clone());
        true
    }
    /// Shows matching labels and their ancestors; paths to matches open automatically.
    pub fn filter(&mut self, keep: impl Fn(&str) -> bool) {
        let mut shown = HashSet::new();
        for i in 0..self.nodes.len() {
            if !keep(&self.nodes[i].label) {
                continue;
            }
            shown.insert(i);
            let mut at = self.nodes[i].parent;
            while let Some(j) = at {
                shown.insert(j);
                self.nodes[j].expanded = true;
                at = self.nodes[j].parent;
            }
        }
        self.shown = Some(shown);
        self.rebuild();
    }
    /// Removes a filter, keeping current expansion state.
    pub fn clear_filter(&mut self) {
        self.shown = None;
        self.rebuild();
    }
    /// Sorts each sibling set with natural filename ordering.
    pub fn sort_natural(&mut self) {
        let nodes = &self.nodes;
        self.roots
            .sort_by(|&a, &b| natural_cmp(&nodes[a].label, &nodes[b].label));
        let orders: Vec<Vec<usize>> = nodes
            .iter()
            .map(|n| {
                let mut c = n.children.clone();
                c.sort_by(|&a, &b| natural_cmp(&nodes[a].label, &nodes[b].label));
                c
            })
            .collect();
        for (n, c) in self.nodes.iter_mut().zip(orders) {
            n.children = c;
        }
        self.rebuild();
    }
    fn reveal_cursor(&self) {
        if let Some(k) = &self.selection.cursor {
            self.rows.reveal(k.clone());
        }
    }
    fn activate(&mut self) -> Option<Arc<str>> {
        let key = self.selection.cursor.clone()?;
        let index = self.nodes.iter().position(|node| node.key == key)?;
        if self.branch(index) {
            self.set_expanded(&key, !self.nodes[index].expanded);
            None
        } else {
            Some(key)
        }
    }
    fn branch(&self, index: usize) -> bool {
        self.nodes[index].children.iter().any(|child| {
            self.shown
                .as_ref()
                .is_none_or(|shown| shown.contains(child))
        })
    }
    fn key(&mut self, key: KeyCode, mods: Modifiers, page: usize) -> bool {
        let at = self
            .selection
            .cursor
            .as_ref()
            .and_then(|k| self.keys.iter().position(|v| v == k));
        if key == KeyCode::LEFT || key == KeyCode::RIGHT {
            let Some(r) = at else {
                if !self.keys.is_empty() {
                    self.selection.click(&self.keys, 0, mods);
                    self.reveal_cursor();
                }
                return true;
            };
            let i = self.visible[r];
            if key == KeyCode::RIGHT {
                if !self.branch(i) {
                    return true;
                }
                if !self.nodes[i].expanded {
                    self.nodes[i].expanded = true;
                    self.rebuild();
                } else if r + 1 < self.keys.len() {
                    self.selection.click(&self.keys, r + 1, mods);
                }
            } else if self.nodes[i].expanded && self.branch(i) {
                let k = self.nodes[i].key.clone();
                self.set_expanded(&k, false);
            } else if let Some(parent) = self.nodes[i].parent
                && let Some(r) = self.visible.iter().position(|&v| v == parent)
            {
                self.selection.click(&self.keys, r, mods);
            }
            self.reveal_cursor();
            true
        } else {
            let used = self.selection.navigate(&self.keys, key, mods, page);
            if used {
                self.reveal_cursor();
            }
            used
        }
    }
}
/// Compares file-like names without parsing digit runs into bounded integers.
/// Equal numeric runs use digit count as a deterministic leading-zero tie-break.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let (mut a, mut b) = (a.as_str(), b.as_str());
    while !a.is_empty() && !b.is_empty() {
        let ca = a.chars().next().unwrap();
        let cb = b.chars().next().unwrap();
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let na = a.bytes().take_while(u8::is_ascii_digit).count();
            let nb = b.bytes().take_while(u8::is_ascii_digit).count();
            let va = a[..na].trim_start_matches('0');
            let vb = b[..nb].trim_start_matches('0');
            let cmp = va
                .len()
                .cmp(&vb.len())
                .then_with(|| va.cmp(vb))
                .then_with(|| na.cmp(&nb));
            if cmp != Ordering::Equal {
                return cmp;
            }
            a = &a[na..];
            b = &b[nb..];
        } else {
            let cmp = ca.cmp(&cb);
            if cmp != Ordering::Equal {
                return cmp;
            }
            a = &a[ca.len_utf8()..];
            b = &b[cb.len_utf8()..];
        }
    }
    a.len().cmp(&b.len())
}
impl<S: 'static> Tree<S> {
    /// Builds cached visible rows with tree keyboard navigation and indexed semantics.
    pub fn virtual_rows(
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        state: &TreeModel,
        model: fn(&mut S) -> &mut TreeModel,
        height: f32,
        open: impl Fn(&mut S, Arc<str>) + 'static,
    ) -> El<S> {
        let key = key.into();
        let body = variable_list(format!("{key}/body"), &state.rows, |r| {
            let i = state.visible[r];
            let n = &state.nodes[i];
            let selected = state.selection.selected.contains(&n.key);
            let mut line = row()
                .h(28.)
                .pad_x((n.level - 1) as f32 * 18.)
                .align(Align::Center)
                .role(Role::TreeItem)
                .label(n.label.clone())
                .level(n.level)
                .size_of_set(n.set_size)
                .position_in_set(n.position)
                .selected(selected)
                .fill(if selected {
                    Paint::Theme(|t| t.color.accent_wash)
                } else {
                    Paint::Rgba(Rgba::TRANSPARENT)
                });
            if state.branch(i) {
                let id = n.key.clone();
                let expanded = n.expanded;
                line = line.expanded(expanded).child(
                    icon(if expanded {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    })
                    .size(14.)
                    .w(22.)
                    .key(format!("{key}/toggle/{}", n.key))
                    .on_press(move |s| model(s).set_expanded(&id, !expanded)),
                );
            } else {
                line = line.child(el().w(22.));
            }
            let id = n.key.clone();
            let focus_key = key.clone();
            line.child(text(n.label.clone()))
                .on_press_with(move |s, tx| {
                    let m = model(s);
                    if let Some(r) = m.keys.iter().position(|k| k == &id) {
                        m.selection.click(&m.keys, r, tx.modifiers());
                        tx.focus(focus_key.clone());
                    }
                })
        })
        .h(height);
        let page = (height / 28.).floor().max(1.) as usize;
        let mut root = col()
            .key(key)
            .role(Role::Tree)
            .label(label)
            .focusable()
            .child(body);
        if let Some(cursor) = &state.selection.cursor {
            root = root.active_descendant(cursor.clone());
        }
        root.on_key(move |s, k, tx| {
            let m = model(s);
            if k == KeyCode::ENTER || k == KeyCode::SPACE {
                if let Some(id) = m.activate() {
                    open(s, id);
                }
                return Handled::Yes;
            }
            m.key(k, tx.modifiers(), page).into()
        })
        .on_char(move |s, c, _| {
            let m = model(s);
            let used = m.selection.type_ahead(&m.keys, &m.labels, c);
            if used {
                m.reveal_cursor();
            }
            used.into()
        })
    }
}
