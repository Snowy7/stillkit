//! Controlled editors using el's native editing and focus contracts.
use crate::{Change, Icon, IconButton, LABEL, SECONDARY};
use std::sync::Arc;
use stillkit::TextRole;
use stillkit::el::{
    self, Align, El, Handled, KeyCode, Paint, Transition, col, icon, row, text, token,
};
type Input<S> = Arc<dyn Fn(&mut S, &str)>;
/// A labelled editor. Supply a stable key and rebuild its value from `on_input`.
pub struct TextField<S> {
    key: Arc<str>,
    label: Arc<str>,
    value: Arc<str>,
    placeholder: Arc<str>,
    helper: Arc<str>,
    invalid: bool,
    inside: bool,
    disabled: bool,
    read_only: bool,
    secure: bool,
    revealed: bool,
    search: bool,
    clearable: bool,
    select_all: bool,
    rows: Option<usize>,
    max: Option<usize>,
    count: bool,
    glyph: Option<Icon>,
    input: Option<Input<S>>,
    submit: Option<Input<S>>,
    reveal: Option<Change<S, bool>>,
    accessory: Option<El<S>>,
}
impl<S: 'static> TextField<S> {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            key: "field".into(),
            label: label.into(),
            value: "".into(),
            placeholder: "".into(),
            helper: "".into(),
            invalid: false,
            inside: false,
            disabled: false,
            read_only: false,
            secure: false,
            revealed: false,
            search: false,
            clearable: false,
            select_all: false,
            rows: None,
            max: None,
            count: false,
            glyph: None,
            input: None,
            submit: None,
            reveal: None,
            accessory: None,
        }
    }
    pub fn key(mut self, key: impl Into<Arc<str>>) -> Self {
        self.key = key.into();
        self
    }
    pub fn value(mut self, value: impl Into<Arc<str>>) -> Self {
        self.value = value.into();
        self
    }
    pub fn placeholder(mut self, value: impl Into<Arc<str>>) -> Self {
        self.placeholder = value.into();
        self
    }
    pub fn helper(mut self, value: impl Into<Arc<str>>) -> Self {
        self.helper = value.into();
        self
    }
    /// El has no invalid-state setter; the description announces the error.
    pub fn error(mut self, value: impl Into<Arc<str>>) -> Self {
        self.helper = value.into();
        self.invalid = true;
        self
    }
    pub fn invalid(mut self, value: bool) -> Self {
        self.invalid = value;
        self
    }
    pub fn label_inside(mut self, value: bool) -> Self {
        self.inside = value;
        self
    }
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
    pub fn read_only(mut self, value: bool) -> Self {
        self.read_only = value;
        self
    }
    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }
    /// Reveal is model-owned so rebuilding never resets the toggle.
    pub fn revealed(mut self, value: bool) -> Self {
        self.revealed = value;
        self
    }
    pub fn on_reveal(mut self, f: impl Fn(&mut S, bool) + 'static) -> Self {
        self.reveal = Some(Arc::new(f));
        self
    }
    pub fn icon(mut self, value: Icon) -> Self {
        self.glyph = Some(value);
        self
    }
    /// Show a clear action while the editable value is nonempty.
    pub fn clearable(mut self, value: bool) -> Self {
        self.clearable = value;
        self
    }
    pub fn accessory(mut self, value: El<S>) -> Self {
        self.accessory = Some(value);
        self
    }
    pub fn select_all_on_focus(mut self) -> Self {
        self.select_all = true;
        self
    }
    pub fn rows(mut self, value: usize) -> Self {
        self.rows = Some(value.max(1));
        self
    }
    pub fn max_len(mut self, value: usize) -> Self {
        self.max = Some(value);
        self
    }
    pub fn character_count(mut self, value: bool) -> Self {
        self.count = value;
        self
    }
    pub fn on_input(mut self, f: impl Fn(&mut S, &str) + 'static) -> Self {
        self.input = Some(Arc::new(f));
        self
    }
    pub fn on_submit(mut self, f: impl Fn(&mut S, &str) + 'static) -> Self {
        self.submit = Some(Arc::new(f));
        self
    }
    pub fn build(self) -> El<S> {
        let radius = if self.rows.is_some() {
            token::RADIUS_CARD
        } else {
            token::RADIUS_CONTROL
        };
        let mut editor = if let Some(rows) = self.rows {
            el::text_area(self.key.clone()).rows(rows)
        } else {
            el::text_input(self.key.clone()).h(token::CONTROL)
        };
        let description = if self.invalid {
            format!("Invalid: {}", self.helper)
        } else {
            self.helper.to_string()
        };
        editor = editor
            .value(self.value.clone())
            .placeholder(self.placeholder)
            .label(self.label.clone())
            .description(description)
            .disabled(self.disabled)
            .font(TextRole::Body)
            .color(LABEL)
            .pad_x(10.)
            .pad_y(token::ITEM)
            .grow(1.)
            .radius(radius)
            .motion(token::MOTION_INTERACTIVE);
        if self.secure && !self.revealed {
            editor = editor.secure();
        }
        if self.read_only {
            editor = editor.read_only();
        }
        if self.select_all {
            editor = editor.select_all_on_focus();
        }
        if let Some(max) = self.max {
            editor = editor.max_len(max);
        }
        if let Some(f) = self.input.clone() {
            editor = editor.on_input(move |s, t| f(s, t));
        }
        if let Some(f) = self.submit {
            editor = editor.on_submit(move |s, t| f(s, t));
        }
        if self.search {
            let f = self.input.clone();
            let editable = !self.read_only;
            editor = editor.on_key(move |s, key, _| {
                if key == KeyCode::ESCAPE
                    && editable
                    && let Some(f) = &f
                {
                    f(s, "");
                }
                Handled::No // Runtime also blurs on Escape.
            });
        }
        let mut well = row().align(Align::Center).fill(token::WELL).radius(radius);
        if let Some(glyph) = self.glyph {
            well = well
                .pad_x(10.)
                .gap(token::ITEM)
                .child(icon(glyph).size(token::ICON).color(token::TERTIARY));
        }
        if self.inside && !self.label.is_empty() {
            well = well.child(text(self.label.clone()).color(SECONDARY).pad_x(8.));
        }
        well = well.child(editor);
        if self.clearable
            && !self.value.is_empty()
            && !self.read_only
            && let Some(f) = self.input
        {
            well = well.child(
                IconButton::new(Icon::Close, "Clear")
                    .key(format!("{}/clear", self.key))
                    .on_press(move |s| f(s, ""))
                    .build()
                    .size(28.)
                    .motion(token::MOTION_INTERACTIVE)
                    .transition(Transition::fade()),
            );
        }
        if self.secure
            && let Some(f) = self.reveal
        {
            let revealed = self.revealed;
            well = well.child(
                IconButton::new(
                    Icon::Eye,
                    if revealed {
                        "Hide password"
                    } else {
                        "Show password"
                    },
                )
                .key(format!("{}/reveal", self.key))
                .on_press(move |s| f(s, !revealed))
                .build()
                .size(28.)
                .checked(revealed),
            );
        }
        if let Some(accessory) = self.accessory {
            well = well.child(accessory);
        }
        let mut out = col()
            .key(format!("{}/frame", self.key))
            .gap(token::ITEM)
            .disabled(self.disabled)
            .opacity(if self.disabled { 0.45 } else { 1. });
        if !self.inside && !self.label.is_empty() {
            out = out.child(text(self.label).font(TextRole::Caption).color(SECONDARY));
        }
        out = out.child(well);
        if !self.helper.is_empty() {
            out = out.child(text(self.helper).wrap(3).font(TextRole::Footnote).color(
                if self.invalid {
                    Paint::Theme(|t| {
                        crate::readable(
                            t.color.destructive,
                            crate::surface(t, t.color.surface),
                            4.5,
                        )
                    })
                } else {
                    SECONDARY
                },
            ));
        }
        if self.count {
            let value = match self.max {
                Some(max) => format!("{} / {max}", self.value.chars().count()),
                None => self.value.chars().count().to_string(),
            };
            out = out.child(
                text(value.clone())
                    .key(format!("{}/count", self.key))
                    .label(value)
                    .font(TextRole::Caption)
                    .color(SECONDARY),
            );
        }
        out
    }
}
/// Password editor convenience constructor; configure `revealed` and `on_reveal`.
pub struct SecureField;
impl SecureField {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<S: 'static>(label: impl Into<Arc<str>>) -> TextField<S> {
        TextField::new(label).secure()
    }
}
/// Search editor with clear, Escape clear/blur, and select-all on focus.
pub struct SearchField;
impl SearchField {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<S: 'static>(label: impl Into<Arc<str>>) -> TextField<S> {
        let mut field = TextField::new(label)
            .icon(Icon::Search)
            .select_all_on_focus();
        field.search = true;
        field.clearable = true;
        field
    }
}
/// Multiline editor; scrolling and caret visibility are supplied by el.
pub struct TextArea;
impl TextArea {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<S: 'static>(label: impl Into<Arc<str>>) -> TextField<S> {
        TextField::new(label).rows(3)
    }
}
