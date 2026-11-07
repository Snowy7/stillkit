//! Copying and pasting text through `wl_data_device` (the seat's selection), for `el`'s text
//! fields: [`System`] is the [`crate::el::Clipboard`] `el::run` gives a runtime on Linux.
//!
//! Copy offers the text as `text/plain;charset=utf-8` (and `UTF8_STRING`, `text/plain`) from a
//! `wl_data_source` and writes it when another client asks. Paste reads the current offer
//! through a pipe, waiting at most [`READ_FOR`] for the other client; our own selection is
//! read back directly (no pipe to ourselves). Compile-checked on Windows; untested live.

use std::io::{Read, Write};
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use wayland_client::protocol::{wl_data_device, wl_data_device_manager, wl_data_offer, wl_data_source, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::surface::State;

/// The text types offered and read, best first.
const TYPES: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
/// How long a paste waits for the client that holds the selection.
const READ_FOR: Duration = Duration::from_millis(500);

/// The seat's data device and what it holds.
#[derive(Default)]
pub(crate) struct Clip {
    pub manager: Option<wl_data_device_manager::WlDataDeviceManager>,
    device: Option<wl_data_device::WlDataDevice>,
    /// Offers announced and their types; the selection is one of them.
    offers: Vec<(wl_data_offer::WlDataOffer, Vec<String>)>,
    selection: Option<wl_data_offer::WlDataOffer>,
    /// What we put on the clipboard, while it is still ours.
    own: Option<(wl_data_source::WlDataSource, String)>,
}

/// Makes the seat's data device once both the manager and the seat are bound.
pub(crate) fn start(clip: &mut Clip, seat: Option<&wl_seat::WlSeat>, q: &QueueHandle<State>) {
    if let (None, Some(manager), Some(seat)) = (&clip.device, &clip.manager, seat) {
        clip.device = Some(manager.get_data_device(seat, q, ()));
    }
}

/// The system clipboard, for a runtime on this thread's Wayland connection.
pub struct System;

impl crate::el::Clipboard for System {
    fn get(&mut self) -> Option<String> {
        super::surface::with(|r| {
            if let Some((_, text)) = &r.state.clip.own {
                return Ok(Some(text.clone()));
            }
            let Some(offer) = r.state.clip.selection.clone() else { return Ok(None) };
            let types = r.state.clip.offers.iter().find(|(o, _)| *o == offer).map(|(_, t)| t.clone()).unwrap_or_default();
            let Some(kind) = TYPES.iter().find(|t| types.iter().any(|have| have == *t)) else { return Ok(None) };
            let mut fds = [0; 2];
            // SAFETY: pipe2 fills two descriptors on success; both are owned at once.
            if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
                return Ok(None);
            }
            // SAFETY: fresh descriptors from pipe2, owned once each.
            let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
            offer.receive((*kind).into(), write.as_fd());
            drop(write);
            r.conn.flush()?;
            Ok(read_all(read))
        })
        .ok()
        .flatten()
    }

    fn set(&mut self, text: &str) {
        let _ = super::surface::with(|r| {
            let q = r.queue.handle();
            let serial = r.state.serial;
            let clip = &mut r.state.clip;
            let (Some(manager), Some(device)) = (&clip.manager, &clip.device) else { return Ok(()) };
            let source = manager.create_data_source(&q, ());
            for kind in TYPES {
                source.offer(kind.into());
            }
            device.set_selection(Some(&source), serial);
            if let Some((old, _)) = clip.own.replace((source, text.into())) {
                old.destroy();
            }
            r.conn.flush()?;
            Ok(())
        });
    }
}

/// Reads `fd` to its end, for at most [`READ_FOR`].
fn read_all(fd: OwnedFd) -> Option<String> {
    let end = Instant::now() + READ_FOR;
    let mut file = std::fs::File::from(fd);
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let mut p = libc::pollfd {
            fd: std::os::fd::AsRawFd::as_raw_fd(&file),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized pollfd.
        if unsafe { libc::poll(&mut p, 1, left.as_millis() as i32) } <= 0 {
            break;
        }
        match file.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
    String::from_utf8(out).ok()
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for State {
    fn event(
        a: &mut Self,
        _: &wl_data_device::WlDataDevice,
        e: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_data_device::Event::DataOffer { id } => a.clip.offers.push((id, Vec::new())),
            wl_data_device::Event::Selection { id } => {
                // Offers other than the new selection are done with.
                let clip = &mut a.clip;
                clip.offers.retain(|(o, _)| {
                    let keep = id.as_ref() == Some(o);
                    if !keep {
                        o.destroy();
                    }
                    keep
                });
                clip.selection = id;
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(State, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wl_data_offer::WlDataOffer, ()),
    ]);
}

impl Dispatch<wl_data_offer::WlDataOffer, ()> for State {
    fn event(
        a: &mut Self,
        offer: &wl_data_offer::WlDataOffer,
        e: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = e
            && let Some((_, types)) = a.clip.offers.iter_mut().find(|(o, _)| o == offer)
        {
            types.push(mime_type);
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for State {
    fn event(
        a: &mut Self,
        source: &wl_data_source::WlDataSource,
        e: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            wl_data_source::Event::Send { fd, .. } => {
                if let Some((_, text)) = a.clip.own.as_ref().filter(|(s, _)| s == source) {
                    let _ = std::fs::File::from(fd).write_all(text.as_bytes());
                }
            }
            wl_data_source::Event::Cancelled => {
                if a.clip.own.as_ref().is_some_and(|(s, _)| s == source) {
                    a.clip.own = None;
                }
                source.destroy();
            }
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(State: ignore wl_data_device_manager::WlDataDeviceManager);
