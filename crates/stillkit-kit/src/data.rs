use super::*;
use stillkit::el::{Axis, Handled, KeyCode};
/// Sized, themed viewport with el-owned wheel, keys, focus reveal and scrollbar.
pub struct ScrollView<S>(El<S>);
impl<S: 'static> ScrollView<S> {
    pub fn new(label: impl Into<Arc<str>>, height: f32) -> Self {
        Self(
            col()
                .h(height)
                .scroll(Axis::Vertical)
                .gap(token::GAP)
                .pad(token::PAD)
                .fill(token::WELL)
                .role(Role::ScrollView)
                .label(label),
        )
    }
    pub fn horizontal(mut self) -> Self {
        self.0 = self.0.scroll(Axis::Horizontal);
        self
    }
    pub fn child(mut self, child: El<S>) -> Self {
        self.0 = self.0.child(child);
        self
    }
    pub fn build(self) -> El<S> {
        self.0
    }
}
/// Current model-owned ordering; `on_sort` receives the requested next order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}
/// Equal-width columns with a fixed header above a scrolling, selectable body.
/// Reorder the supplied rows in `on_sort`; row keys preserve selection and focus.
pub struct Table<S> {
    label: Arc<str>,
    columns: Vec<Arc<str>>,
    rows: Vec<(Arc<str>, Vec<Arc<str>>, bool)>,
    height: f32,
    sort: Option<(usize, SortDirection)>,
    on_sort: Option<Change<S, (usize, SortDirection)>>,
    select: Option<Change<S, Arc<str>>>,
}
impl<S: 'static> Table<S> {
    pub fn new(
        label: impl Into<Arc<str>>,
        columns: impl IntoIterator<Item = impl Into<Arc<str>>>,
    ) -> Self {
        Self {
            label: label.into(),
            columns: columns.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
            height: 200.,
            sort: None,
            on_sort: None,
            select: None,
        }
    }
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
    pub fn row(
        mut self,
        key: impl Into<Arc<str>>,
        cells: impl IntoIterator<Item = impl Into<Arc<str>>>,
        selected: bool,
    ) -> Self {
        self.rows.push((
            key.into(),
            cells.into_iter().map(Into::into).collect(),
            selected,
        ));
        self
    }
    pub fn sorted(mut self, column: usize, direction: SortDirection) -> Self {
        self.sort = Some((column, direction));
        self
    }
    pub fn on_sort(mut self, f: impl Fn(&mut S, (usize, SortDirection)) + 'static) -> Self {
        self.on_sort = Some(Arc::new(f));
        self
    }
    pub fn on_select(mut self, f: impl Fn(&mut S, Arc<str>) + 'static) -> Self {
        self.select = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let count = self.columns.len();
        let mut header = row()
            .h(token::CONTROL)
            .pad_x(8.)
            .fill(token::CARD)
            .role(Role::Row);
        for (i, label) in self.columns.into_iter().enumerate() {
            let mut cell = row()
                .w(0.)
                .grow(1.)
                .gap(token::ITEM)
                .align(Align::Center)
                .role(Role::ColumnHeader)
                .label(label.clone())
                .child(text(label).color(LABEL).font(TextRole::Headline));
            if let Some((col, direction)) = self.sort
                && col == i
            {
                cell = cell
                    .value(if direction == SortDirection::Ascending {
                        "Ascending"
                    } else {
                        "Descending"
                    })
                    .child(
                        icon(if direction == SortDirection::Ascending {
                            Icon::ChevronUp
                        } else {
                            Icon::ChevronDown
                        })
                        .size(12.)
                        .color(SECONDARY),
                    );
            }
            if let Some(f) = self.on_sort.clone() {
                let next = if self.sort == Some((i, SortDirection::Ascending)) {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                };
                cell = cell.on_press(move |s| f(s, (i, next)));
            }
            header = header.child(cell.key(format!("column-{i}")));
        }
        let mut body = col()
            .h(self.height)
            .scroll(Axis::Vertical)
            .focus_group(Orientation::Vertical)
            .role(Role::RowGroup);
        for (key, cells, selected) in self.rows {
            let mut line = row()
                .min_h(token::CONTROL)
                .pad_x(8.)
                .align(Align::Center)
                .role(Role::Row)
                .label(
                    cells
                        .iter()
                        .map(|c| c.as_ref())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .selected(selected)
                .motion(token::MOTION_INTERACTIVE)
                .fill(if selected {
                    Paint::Theme(|t| t.color.accent_wash)
                } else {
                    Paint::Rgba(Rgba::TRANSPARENT)
                });
            for i in 0..count {
                let value = cells.get(i).cloned().unwrap_or_else(|| "".into());
                line = line.child(text(value.clone()).w(0.).grow(1.).color(LABEL));
            }
            if let Some(f) = self.select.clone() {
                let id = key.clone();
                line = line.on_press(move |s| f(s, id.clone()));
            }
            body = body.child(line.key(key));
        }
        col()
            .role(Role::Table)
            .label(self.label)
            .child(header.key("header"))
            .child(body.key("body"))
    }
}
/// One model-owned tree node. Stable keys must be unique across the tree.
pub struct TreeNode {
    pub key: Arc<str>,
    pub label: Arc<str>,
    pub expanded: bool,
    pub selected: bool,
    pub children: Vec<TreeNode>,
}
impl TreeNode {
    pub fn new(key: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            expanded: false,
            selected: false,
            children: Vec::new(),
        }
    }
    pub fn child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}
/// Flat visible rows with roving Up/Down focus and Left/Right collapse/expand.
pub struct Tree<S> {
    label: Arc<str>,
    nodes: Vec<TreeNode>,
    toggle: Option<Change<S, (Arc<str>, bool)>>,
    select: Option<Change<S, Arc<str>>>,
}
impl<S: 'static> Tree<S> {
    pub fn new(label: impl Into<Arc<str>>, nodes: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            nodes,
            toggle: None,
            select: None,
        }
    }
    pub fn on_expand(mut self, f: impl Fn(&mut S, (Arc<str>, bool)) + 'static) -> Self {
        self.toggle = Some(Arc::new(f));
        self
    }
    pub fn on_select(mut self, f: impl Fn(&mut S, Arc<str>) + 'static) -> Self {
        self.select = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        fn append<S: 'static>(
            mut root: El<S>,
            nodes: Vec<TreeNode>,
            depth: usize,
            toggle: &Option<Change<S, (Arc<str>, bool)>>,
            select: &Option<Change<S, Arc<str>>>,
        ) -> El<S> {
            for node in nodes {
                let branch = !node.children.is_empty();
                let expanded = node.expanded;
                let mut line = row()
                    .h(token::CONTROL_SMALL)
                    .pad_x(8. + depth as f32 * 18.)
                    .gap(token::ITEM)
                    .align(Align::Center)
                    .role(Role::TreeItem)
                    .label(node.label.clone())
                    .selected(node.selected)
                    .focusable()
                    .motion(token::MOTION_INTERACTIVE)
                    .fill(if node.selected {
                        Paint::Theme(|t| t.color.accent_wash)
                    } else {
                        Paint::Rgba(Rgba::TRANSPARENT)
                    })
                    .child(
                        icon(if branch {
                            if expanded {
                                Icon::ChevronDown
                            } else {
                                Icon::ChevronRight
                            }
                        } else {
                            Icon::Folder
                        })
                        .size(14.)
                        .color(SECONDARY),
                    )
                    .child(text(node.label).color(LABEL));
                if branch {
                    line = line.expanded(expanded);
                }
                let f = toggle.clone();
                let id = node.key.clone();
                line = line.on_key(move |s, key, _| {
                    if branch
                        && (key == KeyCode::LEFT || key == KeyCode::RIGHT)
                        && let Some(f) = &f
                    {
                        f(s, (id.clone(), key == KeyCode::RIGHT));
                        Handled::Yes
                    } else {
                        Handled::No
                    }
                });
                if let Some(f) = select.clone() {
                    let id = node.key.clone();
                    line = line.on_press(move |s| f(s, id.clone()));
                } else if branch && let Some(f) = toggle.clone() {
                    let id = node.key.clone();
                    line = line.on_press(move |s| f(s, (id.clone(), !expanded)));
                }
                root = root.child(line.key(node.key));
                if expanded {
                    root = append(root, node.children, depth + 1, toggle, select);
                }
            }
            root
        }
        append(
            col()
                .role(Role::Tree)
                .label(self.label)
                .focus_group(Orientation::Vertical),
            self.nodes,
            0,
            &self.toggle,
            &self.select,
        )
    }
}
