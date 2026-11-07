//! Editable selection with model-owned query and highlight.
use crate::{Change, Menu, MenuItem, TextField};
use std::sync::Arc;
use stillkit::Role;
use stillkit::el::{self, Axis, El, Handled, KeyCode, col};
/// Shared controlled state for a combobox or command palette. Highlight indexes filtered results.
#[derive(Clone, Default, Debug)]
pub struct QueryState {
    pub query: String,
    pub open: bool,
    pub highlighted: usize,
    pub selected: Option<usize>,
}
pub(crate) fn matches(label: &str, query: &str) -> bool {
    label.to_lowercase().contains(&query.to_lowercase())
}
/// Type to filter, arrows to highlight, Enter to select and Escape to dismiss.
pub struct Combobox<S> {
    key: Arc<str>,
    label: Arc<str>,
    options: Vec<Arc<str>>,
    state: QueryState,
    change: Change<S, QueryState>,
}
impl<S: 'static> Combobox<S> {
    pub fn new(
        label: impl Into<Arc<str>>,
        options: impl IntoIterator<Item = impl Into<Arc<str>>>,
        state: &QueryState,
        change: impl Fn(&mut S, QueryState) + 'static,
    ) -> Self {
        Self {
            key: "combobox".into(),
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            state: state.clone(),
            change: Arc::new(change),
        }
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = key.into();
        self
    }
    pub fn build(mut self) -> El<S> {
        let filtered: Vec<_> = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, s)| matches(s, &self.state.query))
            .map(|(i, s)| (i, s.clone()))
            .collect();
        let selected = self
            .state
            .selected
            .and_then(|i| self.options.get(i))
            .cloned()
            .unwrap_or_default();
        self.state.highlighted = self.state.highlighted.min(filtered.len().saturating_sub(1));
        let mut menu = Menu::new();
        if filtered.is_empty() {
            menu = menu.section("No matches");
        }
        for (position, (index, label)) in filtered.iter().enumerate() {
            let (change, mut next) = (self.change.clone(), self.state.clone());
            next.query = label.to_string();
            next.selected = Some(*index);
            next.open = false;
            menu = menu.item(
                MenuItem::new(label.clone())
                    .key(format!("{}/option-{index}", self.key))
                    .highlighted(position == self.state.highlighted)
                    .on_press(move |s| change(s, next.clone())),
            );
        }
        let (change, state) = (self.change.clone(), self.state.clone());
        let field = TextField::new(self.label.clone())
            .key(format!("{}/input", self.key))
            .value(self.state.query.clone())
            .on_input(move |s, t| {
                let mut next = state.clone();
                next.query = t.into();
                next.open = true;
                next.highlighted = 0;
                change(s, next);
            })
            .build();
        let (close, state) = (self.change.clone(), self.state.clone());
        let overlay = el::popover(menu.build().max_h(240.).scroll(Axis::Vertical))
            .match_width()
            .open(self.state.open)
            .on_dismiss(move |s| {
                let mut next = state.clone();
                next.open = false;
                close(s, next);
            });
        let (change, state, key) = (self.change, self.state, self.key);
        col()
            .key(key.clone())
            .role(Role::ComboBox)
            .label(self.label)
            .value(selected)
            .expanded(state.open)
            .child(field)
            .overlay(overlay)
            .on_key(move |s, k, tx| {
                let mut next = state.clone();
                if k == KeyCode::ESCAPE && next.open {
                    next.open = false;
                } else if k == KeyCode::DOWN || k == KeyCode::UP {
                    if !next.open {
                        next.open = true;
                        next.highlighted = 0;
                    } else if !filtered.is_empty() {
                        next.highlighted = if k == KeyCode::DOWN {
                            (next.highlighted + 1) % filtered.len()
                        } else {
                            (next.highlighted + filtered.len() - 1) % filtered.len()
                        };
                    }
                    if let Some((i, _)) = filtered.get(next.highlighted) {
                        tx.scroll_into_view(format!("{key}/option-{i}"));
                    }
                } else if k == KeyCode::ENTER && next.open {
                    if let Some((i, label)) = filtered.get(next.highlighted) {
                        next.selected = Some(*i);
                        next.query = label.to_string();
                        next.open = false;
                    }
                } else {
                    return Handled::No;
                }
                change(s, next);
                Handled::Yes
            })
    }
}
