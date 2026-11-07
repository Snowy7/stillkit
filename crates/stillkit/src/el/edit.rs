//! A text field's editing model ([`super::text_input`], [`super::text_area`]): the text, the
//! selection, undo and redo, and an IME composition, with no layout or drawing. Offsets are
//! byte offsets on grapheme boundaries. The runtime turns keys into [`Cmd`]s ([`command`]),
//! applies them here and moves vertically through the field's laid-out lines itself.

use unicode_segmentation::UnicodeSegmentation;

use super::element::{KeyCode, Modifiers};

/// One editing step a key (or the pointer, or a paste) asks for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    /// Moves the caret (extending the selection with `true`).
    Move(Move, bool),
    /// Deletes the selection, else back one grapheme (or word).
    Backspace { word: bool },
    /// Deletes the selection, else ahead one grapheme (or word).
    Delete { word: bool },
    Insert(String),
    Newline,
    SelectAll,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Submit,
}

/// Where a caret move goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Move {
    Left,
    Right,
    WordLeft,
    WordRight,
    /// The start or end of the visual line.
    Home,
    End,
    /// The start or end of the whole text.
    Start,
    Finish,
    /// A visual line up or down (multi-line fields), or this many lines (Page Up and Down).
    Up(usize),
    Down(usize),
}

/// What a key does in a focused field, or `None` when the field refuses it (it goes on to
/// the field's ancestors' `on_key`, then the defaults). Characters arrive apart from keys
/// ([`super::Runtime::type_text`]); a key that will produce one is still claimed here
/// ([`types`]), so a parent's shortcut on a letter never fires while typing.
pub(crate) fn command(key: KeyCode, m: Modifiers, multiline: bool) -> Option<Cmd> {
    let (ctrl, shift) = (m.ctrl && !m.alt, m.shift);
    Some(match key {
        KeyCode::LEFT if ctrl => Cmd::Move(Move::WordLeft, shift),
        KeyCode::RIGHT if ctrl => Cmd::Move(Move::WordRight, shift),
        KeyCode::LEFT => Cmd::Move(Move::Left, shift),
        KeyCode::RIGHT => Cmd::Move(Move::Right, shift),
        KeyCode::HOME if ctrl => Cmd::Move(Move::Start, shift),
        KeyCode::END if ctrl => Cmd::Move(Move::Finish, shift),
        KeyCode::HOME => Cmd::Move(Move::Home, shift),
        KeyCode::END => Cmd::Move(Move::End, shift),
        KeyCode::UP if multiline && !ctrl => Cmd::Move(Move::Up(1), shift),
        KeyCode::DOWN if multiline && !ctrl => Cmd::Move(Move::Down(1), shift),
        KeyCode::PAGE_UP if multiline => Cmd::Move(Move::Up(PAGE), shift),
        KeyCode::PAGE_DOWN if multiline => Cmd::Move(Move::Down(PAGE), shift),
        KeyCode::BACKSPACE if !m.alt => Cmd::Backspace { word: ctrl },
        KeyCode::DELETE if shift && !ctrl => Cmd::Cut,
        KeyCode::DELETE => Cmd::Delete { word: ctrl },
        KeyCode::INSERT if ctrl && !shift => Cmd::Copy,
        KeyCode::INSERT if shift && !ctrl => Cmd::Paste,
        KeyCode::ENTER if multiline && !ctrl => Cmd::Newline,
        KeyCode::ENTER => Cmd::Submit,
        k if ctrl && k == KeyCode::of('A') => Cmd::SelectAll,
        k if ctrl && k == KeyCode::of('C') => Cmd::Copy,
        k if ctrl && k == KeyCode::of('X') => Cmd::Cut,
        k if ctrl && k == KeyCode::of('V') => Cmd::Paste,
        k if ctrl && shift && k == KeyCode::of('Z') => Cmd::Redo,
        k if ctrl && k == KeyCode::of('Z') => Cmd::Undo,
        k if ctrl && k == KeyCode::of('Y') => Cmd::Redo,
        _ => return None,
    })
}

/// Visual lines a Page Up or Page Down moves in a multi-line field.
const PAGE: usize = 8;

/// Whether `key` will produce a character (claimed by a focused field, which types it when the
/// character arrives): letters, digits, space and the punctuation keys, without Ctrl or Alt
/// (Ctrl+Alt is AltGr on Windows layouts).
pub(crate) fn types(key: KeyCode, m: Modifiers) -> bool {
    let printable = matches!(key.0, 0x20 | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2);
    printable && (m.ctrl == m.alt) && !m.meta
}

/// A state undo goes back to.
#[derive(Debug, Clone, PartialEq)]
struct Snap {
    text: String,
    anchor: usize,
    caret: usize,
}

/// An editor's state at one moment ([`Editor::mark`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mark {
    snap: Snap,
    undo: usize,
}

impl Mark {
    pub fn text(&self) -> &str {
        &self.snap.text
    }
}

/// What the last edit was, so typing (or deleting) runs undo as one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    Typing,
    Deleting,
}

/// A field's text and selection, with its history.
#[derive(Debug, Clone, Default)]
pub(crate) struct Editor {
    pub text: String,
    /// Where the selection started; the caret is its other end.
    pub anchor: usize,
    pub caret: usize,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    run: Option<Run>,
    /// An IME composition at the caret: its text and its own caret (bytes into it).
    pub preedit: Option<(String, usize)>,
    /// Characters the text may hold (`max_len`).
    pub max: Option<usize>,
}

/// How many steps undo keeps.
const HISTORY: usize = 100;

impl Editor {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            anchor: text.len(),
            caret: text.len(),
            ..Self::default()
        }
    }

    /// The selection, start to end.
    pub fn selection(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    pub fn selected(&self) -> &str {
        &self.text[self.selection()]
    }

    /// Puts the caret at `at` (snapped to a grapheme boundary), extending the selection or not.
    pub fn place(&mut self, at: usize, extend: bool) {
        let at = snap(&self.text, at);
        self.caret = at;
        if !extend {
            self.anchor = at;
        }
        self.run = None;
    }

    /// Selects `range` with the caret at its end.
    pub fn select(&mut self, range: std::ops::Range<usize>) {
        self.anchor = snap(&self.text, range.start);
        self.caret = snap(&self.text, range.end);
        self.run = None;
    }

    /// Replaces the text from outside (the view gave another value), keeping the caret where
    /// it was when it still fits. History stays: undo returns to the field's own edits.
    pub fn set_text(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text = text.into();
        self.anchor = snap(&self.text, self.anchor.min(self.text.len()));
        self.caret = snap(&self.text, self.caret.min(self.text.len()));
        self.preedit = None;
        self.run = None;
    }

    /// Replaces the selection with `insert` (cut to `max` characters, one line unless
    /// `multiline`) as one undo step, unless it continues a run of the same kind. Whether the
    /// text changed.
    fn replace(&mut self, insert: &str, multiline: bool, run: Option<Run>) -> bool {
        let insert = if multiline { insert.replace("\r\n", "\n") } else { insert.replace(['\r', '\n'], " ") };
        let range = self.selection();
        let room = self
            .max
            .map_or(usize::MAX, |max| max.saturating_sub(self.text.chars().count() - self.text[range.clone()].chars().count()));
        let insert: String = insert.chars().take(room).collect();
        if insert.is_empty() && range.is_empty() {
            return false;
        }
        if run.is_none() || self.run != run || run == Some(Run::Typing) && insert.starts_with(char::is_whitespace) {
            self.remember();
        }
        self.text.replace_range(range.clone(), &insert);
        self.caret = range.start + insert.len();
        self.anchor = self.caret;
        self.run = run;
        true
    }

    /// Types `text` over the selection (consecutive typing undoes as one step).
    pub fn type_text(&mut self, text: &str, multiline: bool) -> bool {
        self.replace(text, multiline, Some(Run::Typing))
    }

    /// Deletes the selection, or from the caret to `to` when nothing is selected.
    fn delete_to(&mut self, to: usize) -> bool {
        if self.anchor == self.caret {
            self.anchor = to;
        }
        self.replace("", true, Some(Run::Deleting))
    }

    /// Applies an edit or a move that needs no layout; `Up`/`Down`/`Home`/`End` and the
    /// clipboard are the runtime's. Whether the text changed.
    pub fn apply(&mut self, cmd: &Cmd, multiline: bool) -> bool {
        match cmd {
            Cmd::Move(m, extend) => {
                let collapsed = self.anchor == self.caret;
                let to = match m {
                    // An unextended arrow collapses a selection to its side.
                    Move::Left if !extend && !collapsed => self.selection().start,
                    Move::Right if !extend && !collapsed => self.selection().end,
                    Move::Left => prev(&self.text, self.caret),
                    Move::Right => next(&self.text, self.caret),
                    Move::WordLeft => word_left(&self.text, self.caret),
                    Move::WordRight => word_right(&self.text, self.caret),
                    Move::Start | Move::Home => 0,
                    Move::Finish | Move::End => self.text.len(),
                    Move::Up(_) | Move::Down(_) => return false,
                };
                self.place(to, *extend);
                false
            }
            Cmd::Backspace { word } => {
                let to = if *word { word_left(&self.text, self.caret) } else { prev(&self.text, self.caret) };
                self.delete_to(to)
            }
            Cmd::Delete { word } => {
                let to = if *word { word_right(&self.text, self.caret) } else { next(&self.text, self.caret) };
                self.delete_to(to)
            }
            Cmd::Insert(text) => self.replace(text, multiline, None),
            Cmd::Newline => self.type_text("\n", multiline),
            Cmd::SelectAll => {
                self.select(0..self.text.len());
                false
            }
            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::Copy | Cmd::Cut | Cmd::Paste | Cmd::Submit => false,
        }
    }

    fn snap(&self) -> Snap {
        Snap {
            text: self.text.clone(),
            anchor: self.anchor,
            caret: self.caret,
        }
    }

    fn restore(&mut self, s: Snap) {
        (self.text, self.anchor, self.caret) = (s.text, s.anchor, s.caret);
        self.run = None;
    }

    /// Saves the state before an edit as an undo step (and forgets what was undone).
    fn remember(&mut self) {
        if self.undo.len() == HISTORY {
            self.undo.remove(0);
        }
        self.undo.push(self.snap());
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(self.snap());
        self.restore(s);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(self.snap());
        self.restore(s);
        true
    }

    /// The state the view last agreed to, to go back to if it refuses what comes next.
    pub fn mark(&self) -> Mark {
        Mark {
            snap: self.snap(),
            undo: self.undo.len(),
        }
    }

    /// Goes back to `mark`, forgetting the edits made since (the view refused them).
    pub fn back(&mut self, mark: Mark) {
        self.restore(mark.snap);
        self.undo.truncate(mark.undo);
        self.redo.clear();
        self.preedit = None;
    }

    /// The word (or run of spaces or punctuation) around `at`: a double click.
    pub fn word_at(&self, at: usize) -> std::ops::Range<usize> {
        let at = at.min(self.text.len());
        for (start, word) in self.text.split_word_bound_indices() {
            let end = start + word.len();
            if at < end || end == self.text.len() {
                return start..end;
            }
        }
        at..at
    }

    /// The hard line (paragraph) around `at`: a triple click.
    pub fn line_at(&self, at: usize) -> std::ops::Range<usize> {
        let start = self.text[..at.min(self.text.len())].rfind('\n').map_or(0, |i| i + 1);
        let end = self.text[start..].find('\n').map_or(self.text.len(), |i| start + i);
        start..end
    }

    /// The text as laid out: an IME composition sits at the caret.
    pub fn shown(&self) -> std::borrow::Cow<'_, str> {
        match &self.preedit {
            Some((pre, _)) => {
                let mut s = self.text.clone();
                s.insert_str(self.caret, pre);
                s.into()
            }
            None => self.text.as_str().into(),
        }
    }

    /// Where the caret is drawn in [`shown`](Self::shown).
    pub fn shown_caret(&self) -> usize {
        self.caret + self.preedit.as_ref().map_or(0, |(_, at)| *at)
    }
}

/// `at` moved back to the grapheme boundary at or before it.
pub(crate) fn snap(text: &str, at: usize) -> usize {
    let at = at.min(text.len());
    let mut last = 0;
    for (i, _) in text.grapheme_indices(true) {
        if i > at {
            break;
        }
        last = i;
    }
    if at == text.len() { at } else { last }
}

pub(crate) fn prev(text: &str, at: usize) -> usize {
    text[..at].grapheme_indices(true).next_back().map_or(0, |(i, _)| i)
}

pub(crate) fn next(text: &str, at: usize) -> usize {
    text[at..].graphemes(true).next().map_or(at, |g| at + g.len())
}

/// Ctrl+Left: the start of the word (or punctuation run) before `at`.
pub(crate) fn word_left(text: &str, at: usize) -> usize {
    text.split_word_bound_indices()
        .take_while(|(i, _)| *i < at)
        .filter(|(_, w)| !w.chars().all(char::is_whitespace))
        .last()
        .map_or(0, |(i, _)| i)
}

/// Ctrl+Right (Windows): the start of the next word after `at`, past the spaces after this
/// one; the end of the text after the last.
pub(crate) fn word_right(text: &str, at: usize) -> usize {
    text.split_word_bound_indices()
        .find(|(i, w)| *i > at && !w.chars().all(char::is_whitespace))
        .map_or(text.len(), |(i, _)| i)
}

/// How a secure field shows `text`: one bullet per grapheme, with the byte offset in `text` of
/// each bullet's start (and the end).
pub(crate) fn masked(text: &str) -> (String, Vec<usize>) {
    let mut offsets: Vec<usize> = text.grapheme_indices(true).map(|(i, _)| i).collect();
    offsets.push(text.len());
    ("\u{2022}".repeat(offsets.len() - 1), offsets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_move_like_windows() {
        let t = "hello, big world";
        assert_eq!(word_right(t, 0), 5, "the comma is a word of its own");
        assert_eq!(word_right(t, 5), 7);
        assert_eq!(word_right(t, 7), 11);
        assert_eq!(word_right(t, 11), t.len());
        assert_eq!(word_left(t, t.len()), 11);
        assert_eq!(word_left(t, 11), 7);
        assert_eq!(word_left(t, 8), 7);
        assert_eq!(word_left(t, 7), 5);
        assert_eq!(word_left(t, 0), 0);
    }

    #[test]
    fn graphemes_move_and_delete_whole() {
        let mut e = Editor::new("ae\u{301}🇸🇪");
        e.apply(&Cmd::Backspace { word: false }, false);
        assert_eq!(e.text, "ae\u{301}", "a flag is one grapheme");
        e.apply(&Cmd::Move(Move::Left, false), false);
        assert_eq!(e.caret, 1, "e with its accent is one step");
    }

    #[test]
    fn typing_undoes_by_word_and_max_len_cuts() {
        let mut e = Editor::new("");
        for c in "one two".chars() {
            e.type_text(&c.to_string(), false);
        }
        assert!(e.undo());
        assert_eq!(e.text, "one");
        assert!(e.undo());
        assert_eq!(e.text, "");
        assert!(e.redo());
        assert_eq!(e.text, "one");
        e.max = Some(5);
        e.apply(&Cmd::Insert("abcdef\nghi".into()), false);
        assert_eq!(e.text, "oneab");
    }
}
