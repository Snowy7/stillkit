//! Input methods through text-input-v3 (`zwp_text_input_v3`), for `el`'s text fields: while a
//! field has focus in a surface that has the text-input focus, the input is enabled with the
//! caret's rectangle (candidate windows open there); the compositor's preedit arrives as
//! `Event::Preedit` (read with [`preedit`]) and drawn inline by `el`, its commits as
//! `Event::Char`s. Compile-checked on Windows; untested live (no compositor with an IME here).

use wayland_client::protocol::wl_seat;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::{zwp_text_input_manager_v3, zwp_text_input_v3};

use super::surface::{Event, State, SurfaceId};
use crate::Rect;

/// The seat's text input and what it last told the compositor.
#[derive(Default)]
pub(crate) struct Ime {
    pub manager: Option<zwp_text_input_manager_v3::ZwpTextInputManagerV3>,
    input: Option<zwp_text_input_v3::ZwpTextInputV3>,
    /// The surface with the text-input focus.
    entered: Option<SurfaceId>,
    /// What the app wants: a field focused in this surface, its caret (device px).
    want: Option<(SurfaceId, Rect)>,
    /// What the compositor was last told (enabled, and where).
    sent: Option<Rect>,
    /// Waiting for `done`.
    pending_preedit: Option<(String, i32)>,
    pending_commit: Option<String>,
    /// The composition now: its text and caret (bytes).
    preedit: (String, usize),
}

/// Makes the seat's text input once both the manager and the seat are bound.
pub(crate) fn start(ime: &mut Ime, seat: Option<&wl_seat::WlSeat>, q: &QueueHandle<State>) {
    if let (None, Some(manager), Some(seat)) = (&ime.input, &ime.manager, seat) {
        ime.input = Some(manager.get_text_input(seat, q, ()));
    }
}

/// The focused field of surface `id` and its caret (device px), or none: the input method
/// follows. Sends nothing when nothing changed.
pub fn follow(id: SurfaceId, caret: Option<Rect>) {
    let _ = super::surface::with(|r| {
        let scale = r.state.scale.max(1) as f32;
        let ime = &mut r.state.ime;
        ime.want = caret.map(|c| (id, Rect::new(c.left / scale, c.top / scale, c.width() / scale, c.height() / scale)));
        if apply(ime) {
            r.conn.flush()?;
        }
        Ok(())
    });
}

/// Tells the compositor what changed; whether it sent anything.
fn apply(ime: &mut Ime) -> bool {
    let Some(input) = &ime.input else { return false };
    let now = ime.want.filter(|(id, _)| ime.entered == Some(*id)).map(|(_, r)| r);
    if now == ime.sent {
        return false;
    }
    match now {
        Some(r) => {
            if ime.sent.is_none() {
                input.enable();
                input.set_content_type(
                    zwp_text_input_v3::ContentHint::None,
                    zwp_text_input_v3::ContentPurpose::Normal,
                );
            }
            input.set_cursor_rectangle(r.left as i32, r.top as i32, r.width().max(1.0) as i32, r.height() as i32);
        }
        None => input.disable(),
    }
    input.commit();
    ime.sent = now;
    true
}

/// The composition now: its text and its caret (bytes into it).
pub fn preedit() -> (String, usize) {
    super::surface::with(|r| Ok(r.state.ime.preedit.clone())).unwrap_or_default()
}

impl Dispatch<zwp_text_input_v3::ZwpTextInputV3, ()> for State {
    fn event(
        a: &mut Self,
        _: &zwp_text_input_v3::ZwpTextInputV3,
        e: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwp_text_input_v3::Event as E;
        let ime = &mut a.ime;
        match e {
            E::Enter { surface } => {
                ime.entered = wayland_client::Proxy::data::<SurfaceId>(&surface).copied();
                ime.sent = None;
                apply(ime);
            }
            E::Leave { .. } => {
                // Leaving disables the input on the compositor's side.
                ime.entered = None;
                ime.sent = None;
            }
            E::PreeditString { text, cursor_begin, .. } => ime.pending_preedit = Some((text.unwrap_or_default(), cursor_begin)),
            E::CommitString { text } => ime.pending_commit = text,
            E::Done { .. } => {
                let Some(id) = ime.entered else { return };
                if let Some(text) = ime.pending_commit.take() {
                    a.events.extend(text.chars().map(|c| (id, Event::Char(c))));
                }
                let (text, cursor) = ime.pending_preedit.take().unwrap_or_default();
                let cursor = usize::try_from(cursor).unwrap_or(text.len()).min(text.len());
                let cursor = (0..=cursor).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
                if (text.as_str(), cursor) != (ime.preedit.0.as_str(), ime.preedit.1) {
                    ime.preedit = (text, cursor);
                    a.events.push_back((id, Event::Preedit));
                }
            }
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(State: ignore zwp_text_input_manager_v3::ZwpTextInputManagerV3);
