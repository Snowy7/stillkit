use super::*;
/// A tab bar with automatic arrow selection and one visible panel.
pub struct Tabs<S> {
    label: Arc<str>,
    selected: usize,
    tabs: Vec<(Arc<str>, El<S>)>,
    change: Option<Change<S, usize>>,
}
impl<S: 'static> Tabs<S> {
    pub fn new(label: impl Into<Arc<str>>, selected: usize) -> Self {
        Self {
            label: label.into(),
            selected,
            tabs: Vec::new(),
            change: None,
        }
    }
    pub fn tab(mut self, label: impl Into<Arc<str>>, panel: El<S>) -> Self {
        self.tabs.push((label.into(), panel));
        self
    }
    pub fn on_change(mut self, f: impl Fn(&mut S, usize) + 'static) -> Self {
        self.change = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let count = self.tabs.len();
        let selected = self.selected.min(count.saturating_sub(1));
        let mut bar = row()
            .focus_group(Orientation::Horizontal)
            .select_follows_focus()
            .role(Role::TabList)
            .label(self.label.clone());
        let mut panel = None;
        for (i, (label, content)) in self.tabs.into_iter().enumerate() {
            let mut b = col()
                .grow(1.)
                .w(0.)
                .h(token::CONTROL)
                .align(Align::Center)
                .justify(Justify::Center)
                .role(Role::Tab)
                .label(label.clone())
                .selected(i == selected)
                .child(text(label).color(if i == selected { LABEL } else { SECONDARY }));
            if let Some(f) = self.change.clone() {
                b = b.on_press(move |s| f(s, i));
            }
            bar = bar.child(b.key(format!("tab-{i}")));
            if i == selected {
                panel = Some(
                    content
                        .key(format!("panel-{i}"))
                        .role(Role::TabPanel)
                        .label(self.label.clone())
                        .transition(Transition::fade()),
                );
            }
        }
        let mut n = col().motion(token::MOTION_LAYOUT).gap(token::GAP).child(
            col().child(bar).child(
                row()
                    .h(3.)
                    .child(el().w(0.).grow(selected as f32))
                    .child(
                        el().key("indicator")
                            .w(0.)
                            .grow(if count > 0 { 1. } else { 0. })
                            .h(3.)
                            .radius(2.)
                            .fill(token::ACCENT)
                            .motion(token::MOTION_INTERACTIVE),
                    )
                    .child(el().w(0.).grow(count.saturating_sub(selected + 1) as f32)),
            ),
        );
        if let Some(panel) = panel {
            n = n.child(panel);
        }
        n
    }
}
/// Controlled expandable section with a stable title and content grid.
pub struct Disclosure<S> {
    label: Arc<str>,
    open: bool,
    content: El<S>,
    toggle: Option<Press<S>>,
}
/// Compose independent disclosures in a column to form an accordion.
pub type Accordion<S> = Disclosure<S>;
impl<S: 'static> Disclosure<S> {
    pub fn new(label: impl Into<Arc<str>>, open: bool, content: El<S>) -> Self {
        Self {
            label: label.into(),
            open,
            content,
            toggle: None,
        }
    }
    pub fn on_toggle(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.toggle = Some(Box::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        self.build_inset(0.)
    }
    fn build_inset(self, inset: f32) -> El<S> {
        let mut trigger = row()
            .h(token::CONTROL)
            .gap(token::ITEM)
            .pad_x(inset)
            .radius(token::RADIUS_CARD)
            .align(Align::Center)
            .role(Role::Button)
            .label(self.label.clone())
            .expanded(self.open)
            .child(
                icon(if self.open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                })
                .size(token::ICON)
                .color(SECONDARY)
                .motion(token::MOTION_OPEN)
                .key("chevron"),
            )
            .child(text(self.label).color(LABEL));
        if let Some(f) = self.toggle {
            trigger = trigger.on_press(f);
        }
        let mut n = col()
            .motion(token::MOTION_OPEN)
            .child(trigger.key("trigger"));
        if self.open {
            n = n.child(
                row()
                    .key("content")
                    .transition(Transition::fade())
                    .pad_x(inset)
                    .child(el().w(Len::Theme(|t| t.metrics.icon + t.metrics.item)))
                    .child(self.content.w(0.).grow(1.)),
            );
        }
        n
    }
}
/// Navigation sections; items carry caller-owned stable keys and selection.
pub struct Sidebar<S> {
    root: El<S>,
}
impl<S: 'static> Sidebar<S> {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            root: col()
                .gap(token::ITEM)
                .pad(token::ITEM)
                .fill(token::WELL)
                .role(Role::List)
                .label(label)
                .focus_group(Orientation::Vertical)
                .select_follows_focus(),
        }
    }
    pub fn section(mut self, label: impl Into<Arc<str>>) -> Self {
        self.root = self.root.child(SectionHeader::new(label).build());
        self
    }
    pub fn item(
        mut self,
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        glyph: Icon,
        selected: bool,
        f: impl Fn(&mut S) + 'static,
    ) -> Self {
        self.root = self.root.child(
            ListRow::new(label)
                .lead(icon(glyph).size(token::ICON).color(SECONDARY))
                .key(key)
                .on_press(f)
                .build()
                .selected(selected)
                .fill(if selected {
                    Paint::Theme(|t| t.color.accent_wash)
                } else {
                    Paint::Rgba(Rgba::TRANSPARENT)
                }),
        );
        self
    }
    pub fn group(mut self, group: Disclosure<S>) -> Self {
        self.root = self
            .root
            .child(group.build_inset(10.).focus_group(Orientation::Vertical));
        self
    }
    pub fn build(self) -> El<S> {
        self.root
    }
}
/// Roving icon actions with separators. Additional children may be custom controls.
pub struct Toolbar<S> {
    root: El<S>,
}
impl<S: 'static> Toolbar<S> {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            root: row()
                .gap(token::ITEM)
                .pad(token::ITEM)
                .align(Align::Center)
                .fill(token::CARD)
                .radius(token::RADIUS_CONTROL)
                .role(Role::Toolbar)
                .label(label)
                .focus_group(Orientation::Horizontal),
        }
    }
    pub fn action(
        mut self,
        key: impl Into<Arc<str>>,
        label: impl Into<Arc<str>>,
        glyph: Icon,
        f: impl Fn(&mut S) + 'static,
    ) -> Self {
        self.root = self
            .root
            .child(IconButton::new(glyph, label).key(key).on_press(f).build());
        self
    }
    pub fn separator(mut self) -> Self {
        self.root = self.root.child(el().w(1.).h(20.).fill(token::SEPARATOR));
        self
    }
    pub fn child(mut self, child: El<S>) -> Self {
        self.root = self.root.child(child);
        self
    }
    pub fn build(self) -> El<S> {
        self.root
    }
}
