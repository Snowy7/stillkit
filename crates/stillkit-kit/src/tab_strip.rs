//! Stable-id editor tabs. The app applies events to its document model.
use crate::*;
use stillkit::el::{Axis, KeyCode, PointerPhase};

/// One document in a tab strip.
#[derive(Clone)]
pub struct Tab {
    /// Stable document identity, independent of order or title.
    pub id: Arc<str>,
    /// Visible and accessible title.
    pub title: Arc<str>,
    /// Optional leading glyph.
    pub icon: Option<Icon>,
    /// Whether the document has unsaved changes.
    pub dirty: bool,
    /// Whether this document may be closed.
    pub closable: bool,
}
impl Tab {
    /// A closable document, initially clean.
    pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            dirty: false,
            closable: true,
        }
    }
}
/// User intent; closing and tear-out never discard a document inside the kit.
#[derive(Clone, Debug, PartialEq)]
pub enum TabEvent {
    /// Activate this stable id.
    Select(Arc<str>),
    /// Ask the app to close this document (and confirm unsaved changes if needed).
    Close(Arc<str>),
    /// Move a document to a final zero-based index.
    Reorder(Arc<str>, usize),
    /// Release outside the strip; the host may move the document to a new window.
    TornOut(Arc<str>),
    /// Create a document.
    New,
}
/// Transient strip state; keep one per strip in the app model.
#[derive(Clone, Default)]
pub struct TabStripState {
    menu: bool,
    drag: Option<Arc<str>>,
}
impl TabStripState {
    /// Dispatch Ctrl+Tab at the host before Runtime, which reserves Tab for focus traversal.
    /// Returns the selection event; the host applies it using the same callback as the strip.
    pub fn shortcut(
        event: &stillkit::Event,
        mods: stillkit::el::Modifiers,
        tabs: &[Tab],
        selected: &str,
    ) -> Option<TabEvent> {
        if *event != stillkit::Event::Key(0x09) || !mods.ctrl || tabs.is_empty() {
            return None;
        }
        let current = tabs
            .iter()
            .position(|t| t.id.as_ref() == selected)
            .unwrap_or(0);
        let next = if mods.shift {
            (current + tabs.len() - 1) % tabs.len()
        } else {
            (current + 1) % tabs.len()
        };
        Some(TabEvent::Select(tabs[next].id.clone()))
    }
}

/// Scrollable editor tabs, suitable for a `WindowTitlebar` slot.
pub struct TabStrip<S> {
    key: Arc<str>,
    tabs: Vec<Tab>,
    selected: Arc<str>,
    state: TabStripState,
    access: fn(&mut S) -> &mut TabStripState,
    event: Change<S, TabEvent>,
}
impl<S: 'static> TabStrip<S> {
    /// Apply reorder events immediately so keyed neighbours spring into their new places.
    pub fn new(
        key: impl Into<Arc<str>>,
        tabs: impl IntoIterator<Item = Tab>,
        selected: impl Into<Arc<str>>,
        state: &TabStripState,
        access: fn(&mut S) -> &mut TabStripState,
        event: impl Fn(&mut S, TabEvent) + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            tabs: tabs.into_iter().collect(),
            selected: selected.into(),
            state: state.clone(),
            access,
            event: Arc::new(event),
        }
    }
    /// Build tabs, overflow menu and new-document action.
    pub fn build(self) -> El<S> {
        let Self {
            key,
            tabs,
            selected,
            state,
            access,
            event,
        } = self;
        let ids: Vec<_> = tabs.iter().map(|t| t.id.clone()).collect();
        let current = ids.iter().position(|id| *id == selected).unwrap_or(0);
        let count = tabs.len();
        let mut rail = row()
            .min_w(0.)
            .grow(1.)
            .scroll(Axis::Horizontal)
            .role(Role::TabList)
            .label("Documents")
            .focus_group(Orientation::Horizontal)
            .select_follows_focus();
        let mut menu = Menu::new().on_dismiss(move |s| access(s).menu = false);
        for (index, tab) in tabs.into_iter().enumerate() {
            let id = tab.id;
            let tab_key = format!("{key}/tab/{id}");
            let mut item = row()
                .key(tab_key.clone())
                .w(170.)
                .min_w(170.)
                .h(token::CONTROL)
                .pad_x(10.)
                .gap(token::ITEM)
                .align(Align::Center)
                .radius(token::RADIUS_CONTROL)
                .motion(token::MOTION_INTERACTIVE)
                .role(Role::Tab)
                .label(tab.title.clone())
                .description(if tab.dirty {
                    "Unsaved changes"
                } else {
                    "Saved"
                })
                .selected(id == selected)
                .fill(if id == selected {
                    token::CARD
                } else {
                    token::WELL
                });
            if let Some(glyph) = tab.icon {
                item = item.child(icon(glyph).size(token::ICON).color(SECONDARY));
            }
            item = item.child(text(tab.title.clone()).min_w(0.).grow(1.).color(LABEL));
            if tab.dirty {
                item = item.child(el().size(6.).radius(3.).fill(token::ACCENT));
            }
            if tab.closable {
                let (f, close_id) = (event.clone(), id.clone());
                item = item.child(
                    IconButton::new(Icon::Close, format!("Close {}", tab.title))
                        .on_press(move |s| f(s, TabEvent::Close(close_id.clone())))
                        .build()
                        .size(22.),
                );
            }
            let (f, select_id) = (event.clone(), id.clone());
            let select_key = tab_key.clone();
            item = item.on_press_with(move |s, tx| {
                f(s, TabEvent::Select(select_id.clone()));
                tx.scroll_into_view(select_key.clone());
            });
            let (f, drag_id) = (event.clone(), id.clone());
            let closable = tab.closable;
            item = item.on_pointer(move |s: &mut S, e, _| {
                if e.button == stillkit::Button::Middle && e.phase == PointerPhase::Up && closable {
                    f(s, TabEvent::Close(drag_id.clone()));
                    return true;
                }
                if e.button != stillkit::Button::Left {
                    return false;
                }
                match e.phase {
                    PointerPhase::Down => access(s).drag = Some(drag_id.clone()),
                    PointerPhase::Move if access(s).drag.as_ref() == Some(&drag_id) => {
                        // Reorder across the centre of the next slot; the app rebuilds the order.
                        let shift = (e.local.x / 170.).floor() as isize;
                        let to = (index as isize + shift).clamp(0, count.saturating_sub(1) as isize)
                            as usize;
                        if to != index && e.local.y >= -24. && e.local.y <= e.size.1 + 24. {
                            f(s, TabEvent::Reorder(drag_id.clone(), to));
                        } else {
                            return false;
                        }
                    }
                    PointerPhase::Up => {
                        let dragging = access(s).drag.take().is_some();
                        if dragging && (e.local.y < -24. || e.local.y > e.size.1 + 24.) {
                            f(s, TabEvent::TornOut(drag_id.clone()));
                        }
                    }
                    PointerPhase::Cancel => access(s).drag = None,
                    _ => return false,
                }
                true
            });
            let (f, menu_id) = (event.clone(), id);
            menu = menu.item(
                MenuItem::new(tab.title)
                    .radio(menu_id == selected)
                    .on_press(move |s| f(s, TabEvent::Select(menu_id.clone()))),
            );
            rail = rail.child(item);
        }
        let f = event.clone();
        let root_key = key.clone();
        let rail = rail.on_key(move |s, code, tx| {
            let mods = tx.modifiers();
            let backwards = code == KeyCode::LEFT || (code == KeyCode::TAB && mods.shift);
            if ids.is_empty()
                || !(matches!(
                    code,
                    KeyCode::LEFT | KeyCode::RIGHT | KeyCode::HOME | KeyCode::END
                ) || code == KeyCode::TAB && mods.ctrl)
            {
                return false.into();
            }
            let next = match code {
                KeyCode::HOME => 0,
                KeyCode::END => ids.len() - 1,
                _ if backwards => (current + ids.len() - 1) % ids.len(),
                _ => (current + 1) % ids.len(),
            };
            f(s, TabEvent::Select(ids[next].clone()));
            let target = format!("{root_key}/tab/{}", ids[next]);
            tx.focus(target.clone());
            tx.scroll_into_view(target);
            true.into()
        });
        row()
            .key(key)
            .min_w(0.)
            .align(Align::Center)
            .gap(token::ITEM)
            .child(rail)
            .child(
                IconButton::new(Icon::Add, "New tab")
                    .on_press(move |s| event(s, TabEvent::New))
                    .build(),
            )
            .child(
                IconButton::new(Icon::ChevronDown, "All tabs")
                    .on_press(move |s| access(s).menu = !access(s).menu)
                    .build()
                    .overlay(menu.overlay().open(state.menu)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stillkit::{Event, el::Modifiers};
    #[test]
    fn host_tab_shortcut_wraps_by_stable_id_and_ignores_plain_tab() {
        let tabs = [Tab::new("a", "Same title"), Tab::new("b", "Same title")];
        assert_eq!(
            TabStripState::shortcut(&Event::Key(9), Modifiers::NONE, &tabs, "a"),
            None
        );
        assert_eq!(
            TabStripState::shortcut(&Event::Key(9), Modifiers::CTRL, &tabs, "b"),
            Some(TabEvent::Select("a".into()))
        );
        assert_eq!(
            TabStripState::shortcut(
                &Event::Key(9),
                Modifiers::CTRL | Modifiers::SHIFT,
                &tabs,
                "a"
            ),
            Some(TabEvent::Select("b".into()))
        );
        assert_eq!(
            TabStripState::shortcut(&Event::Key(9), Modifiers::CTRL, &[], "a"),
            None
        );
    }
}
