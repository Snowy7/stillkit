//! Searchable command dialog. Attach its overlay to the opening control for focus return.
use crate::combobox::matches;
use crate::overlay_surface::{ShadowLevel, panel};
use crate::{Change, Icon, Menu, MenuItem, Press, QueryState, SearchField};
use std::sync::Arc;
use stillkit::Role;
use stillkit::el::{self, Axis, Handled, KeyCode, Overlay, text, token};
/// One command with a stable key, group, optional icon and shortcut hint.
pub struct Command<S> {
    key: Arc<str>,
    label: Arc<str>,
    group: Arc<str>,
    icon: Option<Icon>,
    shortcut: Option<Arc<str>>,
    run: Press<S>,
}
impl<S> Command<S> {
    pub fn new(
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        run: impl Fn(&mut S) + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            group: "Commands".into(),
            icon: None,
            shortcut: None,
            run: Box::new(run),
        }
    }
    pub fn group(mut self, value: impl Into<Arc<str>>) -> Self {
        self.group = value.into();
        self
    }
    pub fn icon(mut self, value: Icon) -> Self {
        self.icon = Some(value);
        self
    }
    pub fn shortcut(mut self, value: impl Into<Arc<str>>) -> Self {
        self.shortcut = Some(value.into());
        self
    }
}
/// Modal search and grouped results, using the shared controlled query state.
pub struct CommandPalette<S> {
    key: Arc<str>,
    state: QueryState,
    commands: Vec<Command<S>>,
    change: Change<S, QueryState>,
}
impl<S: 'static> CommandPalette<S> {
    pub fn new(state: &QueryState, change: impl Fn(&mut S, QueryState) + 'static) -> Self {
        Self {
            key: "palette".into(),
            state: state.clone(),
            commands: vec![],
            change: Arc::new(change),
        }
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = key.into();
        self
    }
    pub fn command(mut self, command: Command<S>) -> Self {
        self.commands.push(command);
        self
    }
    pub fn build(mut self) -> Overlay<S> {
        let filtered: Vec<_> = self
            .commands
            .into_iter()
            .filter(|c| matches(&c.label, &self.state.query))
            .collect();
        self.state.highlighted = self.state.highlighted.min(filtered.len().saturating_sub(1));
        let mut menu = Menu::new();
        let mut group = Arc::<str>::from("");
        let mut actions = vec![];
        for (i, c) in filtered.into_iter().enumerate() {
            if group != c.group {
                group = c.group.clone();
                menu = menu.section(group.clone());
            }
            let run: Arc<dyn Fn(&mut S)> = Arc::from(c.run);
            actions.push((c.key.clone(), run.clone()));
            let (change, state) = (self.change.clone(), self.state.clone());
            let mut item = MenuItem::new(c.label).key(c.key).on_press(move |s| {
                run(s);
                let mut next = state.clone();
                next.open = false;
                change(s, next);
            });
            if let Some(icon) = c.icon {
                item = item.icon(icon);
            }
            if let Some(hint) = c.shortcut {
                item = item.shortcut(hint);
            }
            // Keep keyboard focus in the search editor.
            item = item.highlighted(i == self.state.highlighted);
            menu = menu.item(item);
        }
        let (change, state) = (self.change.clone(), self.state.clone());
        let input = SearchField::new("")
            .key(format!("{}/input", self.key))
            .placeholder("Search commands")
            .value(self.state.query.clone())
            .icon(Icon::Search)
            .on_input(move |s, t| {
                let mut next = state.clone();
                next.query = t.into();
                next.highlighted = 0;
                change(s, next);
            })
            .build();
        let (change, state) = (self.change.clone(), self.state.clone());
        let empty = actions.is_empty();
        let content = panel(ShadowLevel::Popover)
            .key(self.key)
            .w(360.)
            .pad(token::PAD)
            .gap(token::GAP)
            .role(Role::Dialog)
            .label("Command palette")
            .child(input)
            .child(menu.build().max_h(250.).scroll(Axis::Vertical))
            .children(if empty {
                Some(text("No commands found").color(token::SECONDARY))
            } else {
                None
            })
            .on_key(move |s, k, tx| {
                let mut next = state.clone();
                if k == KeyCode::DOWN || k == KeyCode::UP {
                    if !actions.is_empty() {
                        next.highlighted = if k == KeyCode::DOWN {
                            (next.highlighted + 1) % actions.len()
                        } else {
                            (next.highlighted + actions.len() - 1) % actions.len()
                        };
                        tx.scroll_into_view(actions[next.highlighted].0.clone());
                    }
                } else if k == KeyCode::ENTER {
                    if let Some((_, run)) = actions.get(next.highlighted) {
                        run(s);
                        next.open = false;
                    }
                } else {
                    return Handled::No;
                }
                change(s, next);
                Handled::Yes
            });
        let (change, state) = (self.change, self.state);
        el::dialog(content).open(state.open).on_dismiss(move |s| {
            let mut next = state.clone();
            next.open = false;
            change(s, next);
        })
    }
}
