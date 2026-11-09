//! Text fields in the runtime ([`super::super::text_input`], [`super::super::text_area`]): focus,
//! keys and characters (a focused field gets them before its ancestors' `on_key`), pointer
//! selection, the clipboard, IME composition and the caret's blink. Editing changes only the
//! field's retained [`Field`]; the view runs when an `on_input` handler takes the new text.

use std::time::Duration;

use super::super::edit::{Cmd, Move, command, types};
use super::super::element::{FieldSpec, Handled, KeyCode, Modifiers, Tx, What};
use super::super::field::{Field, Look};
use super::super::token;
use super::{Action, Runtime, each, element, flatten};
use crate::{HitId, Rect};

/// Presses closer than this in time and space count as one double (or triple) click.
const MULTI_CLICK: Duration = Duration::from_millis(500);

/// A press selecting in a field: its identity, how many clicks, and the word or line it
/// started on (a drag after a double click extends by words).
#[derive(Debug, Clone)]
pub(super) struct Selecting {
    id: u64,
    clicks: u8,
    from: std::ops::Range<usize>,
}

impl<S> Runtime<S> {
    /// The field with focus, by identity, with its options.
    pub(super) fn focused_field(&self) -> Option<(usize, u64, &FieldSpec)> {
        let i = self.ix.focus.and_then(|hit| self.laid.iter().position(|l| l.hit == hit))?;
        let el = flatten(&self.root).get(i).copied()?;
        let spec = el.field().filter(|_| !el.sem.disabled)?;
        Some((i, self.laid[i].id, spec))
    }

    /// Keeps a [`Field`] for every field laid out: its look in device px, its text
    /// reconciled with the view's value (a refused edit goes back), and its lines.
    pub(super) fn sync_fields(&mut self) {
        let (theme, s) = (&self.theme, self.scale);
        let mut seen = Vec::new();
        let fields = &mut self.fields;
        let refused = &mut false;
        each(&self.root, &self.laid, &mut |el, i, _| {
            let Some(spec) = el.field() else { return };
            let l = self.laid[i];
            seen.push(l.id);
            let field = fields.entry(l.id).or_insert_with(|| Field::new(spec.value.as_deref().unwrap_or("")));
            field.ed.max = spec.max_len;
            if let Some(value) = &spec.value
                && **value != field.ed.text
            {
                match field.taken.take() {
                    Some(mark) if mark.text() == &**value => {
                        field.ed.back(mark);
                        *refused = true;
                    }
                    _ => field.ed.set_text(value),
                }
            }
            field.value = spec.value.clone();
            field.taken = Some(field.ed.mark());
            let pad = |k: usize| el.sizing.pad[k].map_or(0.0, |p| (p.resolve(theme) * s).round());
            let r = l.rect;
            let content = Rect::new(
                r.left + pad(3),
                r.top + pad(0),
                (r.width() - pad(1) - pad(3)).max(0.0),
                (r.height() - pad(0) - pad(2)).max(0.0),
            );
            field.look = Some(Look {
                style: spec.face.style(theme, s),
                line: spec.face.line(theme, s),
                ink: el.look.color.unwrap_or(token::LABEL).resolve(theme),
                multiline: spec.multiline,
                secure: spec.secure,
                rect: r,
                content,
            });
            field.reshape(&mut self.ui.text);
        });
        self.fields.retain(|id, _| seen.contains(id));
        if *refused {
            self.stale_paint = true;
        }
    }

    /// A text area's content length (device px, its padding included), for its scroll view.
    pub(super) fn field_extent(&self, id: u64) -> Option<(f32, f32)> {
        let f = self.fields.get(&id)?;
        let look = f.look.as_ref().filter(|l| l.multiline)?;
        let pad = look.rect.height() - look.content.height();
        Some((look.rect.width(), f.shape.height() + pad))
    }

    /// Focus moved onto or off a field: the caret's blink restarts, `select_all_on_focus`
    /// selects, a field left behind drops its composition.
    pub(super) fn sync_focus(&mut self) {
        // A view without fields pays nothing per frame.
        if self.fields.is_empty() && self.typing.is_none() {
            return;
        }
        let now = self.focused_field().map(|(_, id, spec)| (id, spec.select_all));
        if now.map(|n| n.0) == self.typing {
            return;
        }
        if let Some(old) = self.typing.and_then(|id| self.fields.get_mut(&id)) {
            old.ed.preedit = None;
            old.goal = None;
        }
        self.typing = now.map(|n| n.0);
        if let Some((id, all)) = now
            && let Some(f) = self.fields.get_mut(&id)
        {
            if all {
                let len = f.ed.text.len();
                f.ed.select(0..len);
            }
            f.touch(self.clock);
        }
        self.stale_paint = true;
    }

    /// A key for the focused field: `Some(true)` used, `Some(false)` refused (it bubbles),
    /// `None` when no field has focus.
    pub(super) fn field_key(&mut self, key: KeyCode) -> Option<bool> {
        let (_, id, spec) = self.focused_field()?;
        let (multiline, read_only, secure) = (spec.multiline, spec.read_only, spec.secure);
        // A field typing through RichEdit (Windows) got the keys it uses already.
        #[cfg(windows)]
        if self.fields.get(&id).is_some_and(|f| f.native.is_some()) {
            return Some(false);
        }
        let Some(cmd) = command(key, self.mods, multiline) else {
            return Some(types(key, self.mods));
        };
        Some(self.field_cmd(id, cmd, read_only, secure))
    }

    /// Runs `cmd` on the field `id`; whether it was used.
    pub(super) fn field_cmd(&mut self, id: u64, cmd: Cmd, read_only: bool, secure: bool) -> bool {
        let Some(f) = self.fields.get_mut(&id) else { return false };
        let multiline = f.look.as_ref().is_some_and(|l| l.multiline);
        let changed = match cmd {
            Cmd::Submit => {
                let hit = self.laid.iter().find(|l| l.id == id).map(|l| l.hit);
                let has = hit.and_then(|h| element(&self.root, &self.laid, h)).is_some_and(|el| el.on_submit.is_some());
                if let (Some(hit), true) = (hit, has) {
                    self.queue.push(Action::Submit(hit, f.ed.text.clone()));
                }
                return has;
            }
            Cmd::Copy | Cmd::Cut if secure => return true,
            Cmd::Copy => {
                if !f.ed.selected().is_empty() {
                    self.clipboard.set(f.ed.selected());
                }
                return true;
            }
            Cmd::Cut if read_only => return true,
            Cmd::Cut => {
                if f.ed.selected().is_empty() {
                    return true;
                }
                self.clipboard.set(f.ed.selected());
                f.ed.apply(&Cmd::Insert(String::new()), multiline)
            }
            Cmd::Paste if read_only => return true,
            Cmd::Paste => match self.clipboard.get() {
                Some(text) => f.ed.apply(&Cmd::Insert(text), multiline),
                None => return true,
            },
            Cmd::Move(m @ (Move::Up(_) | Move::Down(_) | Move::Home | Move::End), extend) => {
                let at = f.ed.caret;
                let n = f.shape.line_of(at);
                let lines = &f.shape.lines;
                let to = match m {
                    Move::Home => lines.get(n).map_or(0, |l| l.start),
                    Move::End => lines.get(n).map_or(f.ed.text.len(), |l| l.end),
                    Move::Up(k) | Move::Down(k) => {
                        let x = f.goal.unwrap_or_else(|| lines.get(n).map_or(0.0, |l| l.x(at)));
                        f.goal = Some(x);
                        let to = if matches!(m, Move::Up(_)) { n.checked_sub(k) } else { Some(n + k) };
                        match to.and_then(|t| lines.get(t).or(lines.last().filter(|_| t >= lines.len()))) {
                            Some(l) if to.is_some_and(|t| t < lines.len()) => l.at(x),
                            Some(_) => f.ed.text.len(),
                            None => 0,
                        }
                    }
                    _ => unreachable!(),
                };
                f.ed.place(to, extend);
                false
            }
            Cmd::Backspace { .. } | Cmd::Delete { .. } | Cmd::Newline | Cmd::Insert(_) | Cmd::Undo | Cmd::Redo if read_only => {
                return true;
            }
            cmd => {
                let moved = matches!(cmd, Cmd::Move(..));
                let changed = f.ed.apply(&cmd, multiline);
                if moved {
                    f.goal = None;
                }
                changed
            }
        };
        self.edited(id, changed);
        true
    }

    /// After an edit or a caret move in field `id`: lay it out, keep the caret in sight,
    /// restart the blink, and hand a changed text to `on_input` (or, with a value and no
    /// handler, refuse it at once).
    fn edited(&mut self, id: u64, changed: bool) {
        let Some(f) = self.fields.get_mut(&id) else { return };
        f.touch(self.clock);
        if changed {
            f.goal = None;
        }
        f.reshape(&mut self.ui.text);
        f.follow();
        self.stale_paint = true;
        if changed {
            let hit = self.laid.iter().find(|l| l.id == id).map(|l| l.hit);
            let handler = hit.and_then(|h| element(&self.root, &self.laid, h)).is_some_and(|el| el.on_input.is_some());
            let f = self.fields.get_mut(&id).expect("the field edited");
            match (hit, handler) {
                (Some(hit), true) => self.queue.push(Action::Input(hit, f.ed.text.clone())),
                // A value with nothing to take edits: the view keeps it.
                _ if f.value.is_some() => {
                    if let Some(mark) = f.taken.clone() {
                        f.ed.back(mark);
                        f.reshape(&mut self.ui.text);
                        f.follow();
                    }
                }
                _ => {}
            }
        }
        self.reveal_caret(id);
    }

    /// Scrolls a text area just enough to show its caret's line.
    fn reveal_caret(&mut self, id: u64) {
        let Some(f) = self.fields.get(&id) else { return };
        let Some(look) = f.look.as_ref().filter(|l| l.multiline) else { return };
        let caret = f.caret(self.scale);
        let (top, pad) = (caret.top - look.rect.top, look.content.top - look.rect.top);
        let spring = self.scroll_spring();
        let t = self.anim.t;
        if let Some(view) = self.scrolls.get_mut(&id)
            && view.reveal((top - pad).max(0.0), caret.bottom - look.rect.top + pad, spring, t)
        {
            self.scrolling = true;
            self.stale_paint = true;
        }
    }

    /// Characters typed (a key's text, or an IME's commit): to the focused field, else up
    /// through the focused element's ancestors' [`El::on_char`](super::super::El::on_char).
    /// Control characters are dropped (their keys did their work).
    pub fn type_text(&mut self, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_control() || *c == '\n').collect();
        if text.is_empty() {
            return;
        }
        if let Some((_, id, spec)) = self.focused_field() {
            if spec.read_only {
                return;
            }
            let multiline = spec.multiline;
            let Some(f) = self.fields.get_mut(&id) else { return };
            f.ed.preedit = None;
            let changed = f.ed.type_text(&text, multiline);
            self.edited(id, changed);
            return;
        }
        for c in text.chars() {
            self.char_up(c);
        }
    }

    /// Offers `c` to the `on_char` of the focused element and its ancestors.
    fn char_up(&mut self, c: char) {
        let focused = self.ix.focus.and_then(|hit| self.index_of_hit(hit));
        let mut at = Some(focused.unwrap_or(0));
        let flat = flatten(&self.root);
        while let Some(i) = at.filter(|&i| i < self.laid.len()) {
            if let Some(handler) = flat.get(i).and_then(|el| el.on_char.as_ref()) {
                let mut tx = Tx {
                    mods: self.mods,
                    ..Tx::default()
                };
                if handler(&mut self.state, c, &mut tx) == Handled::Yes {
                    drop(flat);
                    self.stale_view = true;
                    self.apply(tx, true);
                    return;
                }
            }
            at = self.laid[i].parent;
        }
    }

    /// An IME composition in the focused field (Linux's text-input; Windows' RichEdit draws
    /// its own): `text` drawn at the caret, underlined, with its own caret `cursor` bytes in.
    /// An empty text ends it; the commit comes through [`type_text`](Self::type_text).
    pub fn preedit(&mut self, text: &str, cursor: usize) {
        let Some((_, id, spec)) = self.focused_field() else { return };
        if spec.read_only {
            return;
        }
        let multiline = spec.multiline;
        let Some(f) = self.fields.get_mut(&id) else { return };
        if f.ed.preedit.is_none() && !text.is_empty() && !f.ed.selection().is_empty() {
            // A composition replaces the selection, as typing does.
            f.ed.type_text("", multiline);
        }
        f.ed.preedit = (!text.is_empty()).then(|| (text.to_owned(), cursor.min(text.len())));
        self.edited(id, false);
    }

    /// Where the focused field's caret is (device px in the surface): IME candidate windows
    /// open there.
    pub fn caret_rect(&self) -> Option<Rect> {
        let id = self.typing?;
        let f = self.fields.get(&id)?;
        let by = self.scrolls.get(&id).map_or(0.0, |v| v.offset(self.anim.t));
        Some(f.caret(self.scale).offset(0.0, -by))
    }

    /// The clipboard text fields cut, copy and paste through ([`super::super::clipboard`]).
    pub fn set_clipboard(&mut self, clipboard: impl super::super::Clipboard + 'static) {
        self.clipboard = Box::new(clipboard);
    }

    /// A press, a drag or a release of the primary button, for the field under it: a click
    /// places the caret, a double click selects a word, a triple click the line; dragging
    /// extends by the same unit. Whether a field took it.
    pub(super) fn field_pointer(&mut self, event: &crate::Event) -> bool {
        use crate::{Button, Event};
        match *event {
            Event::Press(p, Button::Left) => {
                let Some(hit) = self.ix.pressed else { return false };
                let Some((id, _)) = self.field_of_hit(hit) else { return false };
                let now = self.clock;
                let scroll_y = self.scrolls.get(&id).map_or(0.0, |v| v.offset(self.anim.t));
                let s = self.scale;
                let Some(f) = self.fields.get_mut(&id) else { return false };
                let clicks = match f.clicks {
                    Some((at, last, n))
                        if now.saturating_sub(at) <= MULTI_CLICK
                            && (last.x - p.at.x).abs() <= 4.0 * s
                            && (last.y - p.at.y).abs() <= 4.0 * s =>
                    {
                        n % 3 + 1
                    }
                    _ => 1,
                };
                f.clicks = Some((now, p.at, clicks));
                let at = f.hit(p.at, scroll_y);
                f.ed.preedit = None;
                let from = match clicks {
                    1 => at..at,
                    2 => f.ed.word_at(at),
                    _ => f.ed.line_at(at),
                };
                f.ed.select(from.clone());
                self.selecting = Some(Selecting { id, clicks, from });
                self.edited(id, false);
                true
            }
            Event::Move(p) => {
                let Some(sel) = self.selecting.clone() else { return false };
                let scroll_y = self.scrolls.get(&sel.id).map_or(0.0, |v| v.offset(self.anim.t));
                let Some(f) = self.fields.get_mut(&sel.id) else { return false };
                let at = f.hit(p.at, scroll_y);
                let unit = match sel.clicks {
                    1 => at..at,
                    2 => f.ed.word_at(at),
                    _ => f.ed.line_at(at),
                };
                let before = f.ed.selection();
                if at < sel.from.start {
                    f.ed.select(sel.from.end..unit.start);
                } else {
                    f.ed.select(sel.from.start..unit.end.max(sel.from.end));
                }
                if f.ed.selection() != before {
                    self.edited(sel.id, false);
                }
                true
            }
            Event::Release(..) | Event::CaptureLost | Event::Cancelled | Event::Leave => {
                self.selecting = None;
                false
            }
            _ => false,
        }
    }

    /// The field whose hit item is `hit`, by identity.
    fn field_of_hit(&self, hit: HitId) -> Option<(u64, usize)> {
        let i = self.laid.iter().position(|l| l.hit == hit)?;
        let el = flatten(&self.root).get(i).copied()?;
        matches!(el.what, What::Field(_)).then(|| (self.laid[i].id, i))
    }

    /// Assistive technology set a field's text: as if it was typed over everything.
    pub(super) fn field_set_text(&mut self, hit: HitId, text: &str) -> bool {
        let Some((id, i)) = self.field_of_hit(hit) else { return false };
        let read_only = flatten(&self.root).get(i).and_then(|el| el.field()).is_none_or(|f| f.read_only);
        let Some(f) = self.fields.get_mut(&id).filter(|_| !read_only) else { return false };
        let len = f.ed.text.len();
        f.ed.select(0..len);
        let changed = f.ed.apply(&Cmd::Insert(text.into()), f.look.as_ref().is_some_and(|l| l.multiline));
        self.edited(id, changed);
        true
    }

    /// When the focused field's caret next blinks (none once it holds still, or while the
    /// window is inactive).
    pub(super) fn blink_due(&self) -> Option<Duration> {
        let f = self.fields.get(&self.typing?).filter(|_| self.active)?;
        f.next_blink(self.clock)
    }

    /// Whether the caret is drawn now.
    pub(super) fn caret_on(&self) -> bool {
        self.active && self.typing.and_then(|id| self.fields.get(&id)).is_some_and(|f| f.caret_on(self.clock))
    }

    /// The field's text and selection (anchor, caret), for tests and the `--tree` dump.
    pub(super) fn field_shown(&self, id: u64) -> Option<(String, (usize, usize))> {
        let f = self.fields.get(&id)?;
        Some((f.ed.text.clone(), (f.ed.anchor, f.ed.caret)))
    }
}

/// The modifier keys a key handler sees.
pub(super) fn tx_with(mods: Modifiers) -> Tx {
    Tx {
        mods,
        ..Tx::default()
    }
}

impl<S> Runtime<S> {
    /// The focused field's native text input follows focus; the window host calls it before
    /// each frame. On Windows a RichEdit types into the focused field (the system IME with its
    /// composition inline and candidates at the caret, the system clipboard); on Linux the
    /// compositor's input method (text-input-v3), on macOS the view's input context, is told
    /// the field has focus and where its caret is. Headless runtimes never call it.
    pub fn sync_text_input(&mut self, surface: &crate::Surface) {
        #[cfg(windows)]
        self.sync_rich_edit(surface);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let caret = self.caret_rect().filter(|_| self.typing.is_some() && self.active);
            crate::platform::current::ime::follow(surface.id(), caret);
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        let _ = surface;
    }
}
