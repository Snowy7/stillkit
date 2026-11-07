//! Windows: while a field has focus in a window, the windowless RichEdit (`TextField`) types
//! into it, with the system's IME (composition drawn inline, the candidate window at the
//! caret), caret placement, mouse selection, undo and clipboard. The runtime still owns focus,
//! the key policy (`crate::el::edit::command`, asked by the surface for each key) and drawing
//! order: the field draws through `Op::TextField`, and the caret from `TextField::caret`.
//! Unfocused fields, and every headless runtime, draw and edit through the runtime's own
//! editor. One RichEdit exists at a time, made on focus and dropped on blur.

use super::super::edit::Cmd;
use super::super::field::Field;
use super::{Action, Runtime, element};
use crate::{Rect, Surface, TextField};

/// UTF-16 positions in `text` for the byte range `range`.
fn utf16(text: &str, range: std::ops::Range<usize>) -> std::ops::Range<u32> {
    let at = |b: usize| text[..b.min(text.len())].encode_utf16().count() as u32;
    at(range.start)..at(range.end)
}

/// The byte offset of UTF-16 position `at` in `text`.
fn byte(text: &str, at: u32) -> usize {
    let mut n = 0;
    for (i, c) in text.char_indices() {
        if n >= at {
            return i;
        }
        n += c.len_utf16() as u32;
    }
    text.len()
}

/// Where RichEdit draws: a one-line field's line, centred; a text area's content box.
pub(super) fn native_rect(f: &Field) -> Option<Rect> {
    let look = f.look.as_ref()?;
    let c = look.content;
    Some(if look.multiline {
        c
    } else {
        let top = (c.top + (c.height() - look.line) / 2.0).round();
        Rect::new(c.left, top, c.width(), look.line)
    })
}

impl<S> Runtime<S> {
    pub(super) fn sync_rich_edit(&mut self, surface: &Surface) {
        let want = self.focused_field().map(|(_, id, spec)| (id, spec.secure, spec.read_only));
        // Fields that lost focus give their RichEdit up.
        let mut dropped = false;
        for (id, f) in &mut self.fields {
            if f.native.is_some() && want.is_none_or(|w| w.0 != *id) {
                f.native = None;
                dropped = true;
            }
        }
        if dropped {
            surface.set_text_field(None);
            self.stale_paint = true;
        }
        let Some((id, secure, read_only)) = want else { return };
        let Some(f) = self.fields.get_mut(&id) else { return };
        let (Some(look), Some(rect)) = (f.look.clone(), native_rect(f)) else { return };
        let field = match &f.native {
            Some(field) => field.clone(),
            None => {
                let Ok(field) = TextField::new(surface.hwnd(), rect, look.multiline, look.style.size, look.ink) else {
                    return;
                };
                field.set_el(secure, read_only, f.ed.max);
                field.set_style(&look.style, look.ink);
                let _ = field.set_text(&f.ed.text);
                field.select_range(utf16(&f.ed.text, f.ed.selection()));
                let _ = field.prepare();
                // Only a window that has the keyboard hands it on; otherwise the first click in
                // the field focuses it (the surface does that).
                if foreground(surface) {
                    field.focus();
                }
                surface.set_text_field(Some(field.clone()));
                f.native = Some(field.clone());
                self.stale_paint = true;
                field
            }
        };
        field.set_rect(rect);
        field.set_style(&look.style, look.ink);
        // The view gave the field another text (or refused an edit): RichEdit shows it.
        if field.text().is_ok_and(|t| t != f.ed.text) {
            let _ = field.set_text(&f.ed.text);
            field.select_range(utf16(&f.ed.text, f.ed.selection()));
        }
    }

    /// RichEdit changed the text, the selection or the caret: the editor follows, and a new
    /// text goes to `on_input` as typing would.
    pub(super) fn rich_edit_changed(&mut self) {
        let Some(id) = self.typing else { return };
        let Some(f) = self.fields.get_mut(&id) else { return };
        let Some(field) = f.native.clone() else { return };
        self.stale_paint = true;
        f.touch(self.clock);
        let Ok(text) = field.text() else { return };
        let text = text.replace('\r', "\n");
        let sel = field.selection();
        let changed = text != f.ed.text;
        if changed {
            f.ed.set_text(&text);
        }
        f.ed.select(byte(&text, sel.start)..byte(&text, sel.end));
        f.reshape(&mut self.ui.text);
        if !changed {
            return;
        }
        let hit = self.laid.iter().find(|l| l.id == id).map(|l| l.hit);
        let takes = hit.and_then(|h| element(&self.root, &self.laid, h)).is_some_and(|el| el.on_input.is_some());
        match hit.filter(|_| takes) {
            Some(hit) => self.queue.push(Action::Input(hit, text)),
            // A value with nothing to take edits keeps it (sync_rich_edit puts it back).
            None if f.value.is_some() => {
                if let Some(mark) = f.taken.clone() {
                    f.ed.back(mark);
                }
            }
            None => {}
        }
    }

    /// Enter in a RichEdit field: its `on_submit`, else the key goes on to its ancestors.
    pub(super) fn rich_edit_submit(&mut self) {
        let Some((_, id, spec)) = self.focused_field() else { return };
        let (read_only, secure) = (spec.read_only, spec.secure);
        if !self.field_cmd(id, Cmd::Submit, read_only, secure) {
            self.key_down(crate::el::KeyCode::ENTER);
        }
    }
}

/// Whether `surface`'s window is the foreground window (it has the keyboard).
fn foreground(surface: &Surface) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetForegroundWindow};
    // SAFETY: read-only window queries.
    unsafe { GetForegroundWindow() == GetAncestor(surface.hwnd(), GA_ROOT) }
}

/// Paints a field RichEdit types into: RichEdit's own drawing (text, selection, composition),
/// the placeholder while it is empty, and the caret where RichEdit puts it.
pub(super) fn paint_rich_edit(
    f: &mut crate::Frame,
    field: &Field,
    native: &TextField,
    placeholder: Option<&str>,
    caret: Option<crate::Rgba>,
    hint: crate::Rgba,
) {
    let (Some(rect), Some(look)) = (native_rect(field), field.look.as_ref()) else { return };
    if field.ed.text.is_empty()
        && let Some(p) = placeholder
    {
        f.push(crate::Op::Text {
            rect: Rect::new(rect.left, rect.top, rect.width(), look.line),
            text: p.into(),
            style: look.style.clone(),
            color: hint,
        });
    }
    f.push(crate::Op::TextField {
        rect,
        field: native.clone(),
        revision: native.revision(),
    });
    if let (Some(color), Some(at)) = (caret, native.caret()) {
        f.push(crate::Op::Fill {
            rect: Rect::new(at.left, at.top, f.s.round().max(1.0), at.height()),
            radius: 0.0,
            color,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::WM_CHAR;

    use crate::el::{El, Runtime, col, el, text_input};
    use crate::{Kind, Role, Scheme, Style, Surface, Theme, next_event};

    #[derive(Default)]
    struct Form {
        name: String,
    }

    fn view(s: &Form) -> El<Form> {
        col().pad(8.0).child(
            text_input("name")
                .w(200.0)
                .value(s.name.as_str())
                .on_input(|s: &mut Form, t| s.name = t.chars().filter(|c| *c != '!').collect()),
        )
        .child(el().key("next").focusable().w(10.0).h(10.0))
    }

    /// A hidden window, never focused: RichEdit attaches on focus, types into the view's
    /// string through `TextChanged`, takes a refused character back out, draws through
    /// `Op::TextField`, and goes when focus leaves.
    #[test]
    fn rich_edit_types_into_a_focused_field_and_goes_with_focus() {
        let surface = Surface::new(Kind::Popup { activate: false }, "el text", Role::Window).unwrap();
        let theme = Theme::new(Style::Liquid, Scheme::Dark, crate::theme::SYSTEM_BLUE);
        let mut rt = Runtime::new(Form::default(), view, theme, (240, 60), 1.0).unwrap();
        assert!(rt.click("name"));
        rt.sync_text_input(&surface);
        rt.frame(Duration::ZERO);
        let id = rt.typing.unwrap();
        let field = rt.fields[&id].native.clone().expect("RichEdit attached on focus");
        assert!(field.is_el());
        let pump = |rt: &mut Runtime<Form>| {
            crate::platform::current::host::wait_for(Duration::ZERO);
            while let Some((_, event)) = next_event() {
                rt.handle(&event);
            }
            rt.frame(Duration::from_millis(16));
            rt.sync_text_input(&surface);
        };
        for c in "Hi!".chars() {
            field.send(WM_CHAR, WPARAM(c as usize), LPARAM(1));
            pump(&mut rt);
        }
        pump(&mut rt);
        assert_eq!(rt.state().name, "Hi");
        assert_eq!(field.text().unwrap(), "Hi", "the refused character left RichEdit too");
        assert!(rt.list().ops.iter().any(|op| matches!(op, crate::Op::TextField { .. })));
        rt.tab(false);
        rt.frame(Duration::ZERO);
        rt.sync_text_input(&surface);
        assert!(rt.fields[&id].native.is_none(), "blur drops the RichEdit");
        assert_eq!(rt.presented("name").unwrap().text.as_deref(), Some("Hi"));
    }
}
