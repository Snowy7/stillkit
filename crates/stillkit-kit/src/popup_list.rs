//! Composable listbox content for app-owned completion and mention popovers.
use crate::*;
use stillkit::el::Axis;

/// One selectable popup row, independent of an editor's query or text storage.
pub struct PopupItem {
    /// Stable row identity.
    pub id: Arc<str>,
    /// Visible and accessible row label.
    pub label: Arc<str>,
    /// Optional explanation below the label.
    pub description: Option<Arc<str>>,
    /// Rows marked disabled cannot be picked or focused.
    pub disabled: bool,
}
impl PopupItem {
    /// Create an enabled row without a description.
    pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            disabled: false,
        }
    }
}
/// Non-modal listbox content; attach it to `el::popover` with app-owned open state.
pub struct PopupList<S> {
    key: Arc<str>,
    label: Arc<str>,
    selected: Arc<str>,
    items: Vec<PopupItem>,
    pick: Change<S, Arc<str>>,
}
impl<S: 'static> PopupList<S> {
    /// A controlled selection and a callback that applies the row's stable id.
    pub fn new(
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        selected: impl Into<Arc<str>>,
        pick: impl Fn(&mut S, Arc<str>) + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            selected: selected.into(),
            items: Vec::new(),
            pick: Arc::new(pick),
        }
    }
    /// Append an option. Filter in the app before composing the list.
    pub fn item(mut self, item: PopupItem) -> Self {
        self.items.push(item);
        self
    }
    /// Build a bounded scroll view with arrow/Home/End navigation and Enter activation.
    pub fn build(self) -> El<S> {
        let mut root = col()
            .key(self.key.clone())
            .role(Role::ListBox)
            .label(self.label)
            .focus_group(Orientation::Vertical)
            .max_h(280.)
            .scroll(Axis::Vertical)
            .pad(6.)
            .gap(2.)
            .fill(token::CARD)
            .radius(token::RADIUS_CARD);
        for item in self.items {
            let callback = self.pick.clone();
            let id = item.id;
            let mut node = col()
                .key(format!("{}/{}", self.key, id))
                .pad_x(10.)
                .pad_y(6.)
                .role(Role::ListBoxOption)
                .label(item.label.clone())
                .selected(id == self.selected)
                .radius(token::RADIUS_CONTROL)
                .disabled(item.disabled)
                .fill(if id == self.selected {
                    token::FILL
                } else {
                    Paint::Rgba(Rgba::TRANSPARENT)
                })
                .child(text(item.label).color(LABEL));
            if let Some(description) = item.description {
                node = node
                    .description(description.clone())
                    .child(text(description).font(TextRole::Caption).color(SECONDARY));
            }
            root = root.child(node.on_press(move |s| callback(s, id.clone())));
        }
        root
    }
}
