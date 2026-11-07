//! A form of text fields: a name, a password, a search field with a clear button and a notes
//! area. The view owns every string; the name is capped at 32 characters, Enter in the search
//! field "searches", and the notes wrap and scroll.
//!
//!   cargo run -p stillkit --example el_text
//!   cargo run -p stillkit --example el_text -- --script s.json --snap text.png
//!     with s.json: [{"at": 0.1, "click": "name"}, {"at": 0.2, "type": "Ada Lovelace"},
//!                   {"at": 0.3, "key": "Ctrl+Left"}, {"at": 0.4, "key": "Shift+End"}]

use stillkit::el::{self, Align, App, El, col, row, text, text_area, text_input, token};
use stillkit::{Kind, Role, Scheme, Style, TextRole, Theme};

#[derive(Default)]
pub struct Form {
    pub name: String,
    pub password: String,
    pub search: String,
    pub searched: Option<String>,
    pub notes: String,
}

/// A field's well: the theme's well fill, rounded, padded to a control's height.
fn well<S>(field: El<S>) -> El<S> {
    field.fill(token::WELL).radius(token::RADIUS_SMALL).pad_x(10.0).pad_y(6.0).w_full()
}

fn labelled<S: 'static>(label: &str, field: El<S>) -> El<S> {
    col().gap(4.0).w_full().child(text(label).font(TextRole::Footnote).color(token::SECONDARY)).child(field)
}

pub fn view(s: &Form) -> El<Form> {
    let search = row()
        .key("search-row")
        .align(Align::Center)
        .gap(token::ITEM)
        .w_full()
        .child(
            well(text_input("search"))
                .grow(1.0)
                .label("Search")
                .placeholder("Search notes")
                .select_all_on_focus()
                .value(s.search.as_str())
                .on_input(|s: &mut Form, t| s.search = t.into())
                .on_submit(|s: &mut Form, t| s.searched = Some(t.into())),
        )
        .child(
            row()
                .key("clear")
                .role(Role::Button)
                .label("Clear search")
                .h(token::CONTROL_SMALL)
                .pad_x(12.0)
                .radius(token::RADIUS_CONTROL)
                .fill(token::FILL)
                .align(Align::Center)
                .disabled(s.search.is_empty())
                .on_press(|s: &mut Form| {
                    s.search.clear();
                    s.searched = None;
                })
                .child(text("Clear")),
        );
    let found = match &s.searched {
        Some(q) => format!("{} matches for \u{201c}{q}\u{201d}", s.notes.matches(q.as_str()).count()),
        None => "Enter searches the notes".into(),
    };
    col()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(text("Profile").font(TextRole::Title2))
        .child(labelled(
            "Name",
            well(text_input("name"))
                .label("Name")
                .placeholder("Your name")
                .max_len(32)
                .value(s.name.as_str())
                .on_input(|s: &mut Form, t| s.name = t.into()),
        ))
        .child(labelled(
            "Password",
            well(text_input("password"))
                .label("Password")
                .secure()
                .value(s.password.as_str())
                .on_input(|s: &mut Form, t| s.password = t.into()),
        ))
        .child(labelled("Search", search))
        .child(text(found).key("found").font(TextRole::Footnote).color(token::TERTIARY))
        .child(labelled(
            "Notes",
            well(text_area("notes"))
                .label("Notes")
                .rows(5)
                .placeholder("Anything worth remembering")
                .value(s.notes.as_str())
                .on_input(|s: &mut Form, t| s.notes = t.into()),
        ))
        .child(text(format!("{} characters", s.notes.chars().count())).font(TextRole::Footnote).color(token::TERTIARY))
}

pub fn theme() -> Theme {
    Theme::new(Style::Liquid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
}


fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Profile",
        size: (360.0, 520.0),
        kind: Kind::App { min: (280, 360) },
        theme: theme(),
        state: Form {
            notes: "Text areas wrap at word boundaries and scroll when the text is taller than \
                    the box: the wheel, the overlay scrollbar, and the caret kept in sight."
                .into(),
            ..Form::default()
        },
        view,
    })
}
