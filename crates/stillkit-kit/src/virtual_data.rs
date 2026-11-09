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
