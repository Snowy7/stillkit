//! XDG FileChooser on a private, opt-in session-bus connection. Like the other
//! native picker backends this blocks the caller until the user answers.
use super::{
    a11y_bus::Message,
    services::{
        bus::{Bus, string, word},
        portal::{self, dict, entry},
    },
};
use crate::dialog::Request;
use anyhow::{Result, anyhow};
use std::{
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
pub(crate) fn pick(title: &str, start: Option<&Path>, request: Request<'_>) -> Vec<PathBuf> {
    match choose(title, start, request) {
        Ok(paths) => paths,
        Err(error) => {
            log::debug!("file chooser: {error:#}");
            Vec::new()
        }
    }
}
fn choose(title: &str, start: Option<&Path>, request: Request<'_>) -> Result<Vec<PathBuf>> {
    let mut bus = Bus::connect()?;
    portal::subscribe(&mut bus, "org.freedesktop.portal.Request")?;
    let mut b = Vec::new();
    string(&mut b, "");
    string(&mut b, title);
    dict(&mut b, |b| {
        entry(b, "handle_token", "s", |b| string(b, "file"));
        entry(b, "modal", "b", |b| word(b, 1));
        match request {
            Request::Folder => entry(b, "directory", "b", |b| word(b, 1)),
            Request::Files { multiple } => {
                entry(b, "multiple", "b", |b| word(b, u32::from(multiple)))
            }
            Request::Save { name } => entry(b, "current_name", "s", |b| string(b, name)),
        }
        if let Some(start) = start {
            entry(b, "current_folder", "ay", |b| {
                let bytes = start.as_os_str().as_bytes();
                word(b, (bytes.len() + 1) as u32);
                b.extend_from_slice(bytes);
                b.push(0);
            });
        }
    });
    let method = if matches!(request, Request::Save { .. }) {
        "SaveFile"
    } else {
        "OpenFile"
    };
    let bytes = portal::request(
        &mut bus,
        "org.freedesktop.portal.FileChooser",
        method,
        "ssa{sv}",
        b,
    )?;
    let mut reader = Message::parse(&bytes)
        .and_then(|m| m.body_reader(&bytes, "ua{sv}"))
        .ok_or_else(|| anyhow!("invalid chooser response"))?;
    reader.u32();
    let mut paths = Vec::new();
    reader
        .array(8, |r| {
            r.align(8)?;
            let key = r.str()?;
            let signature = r.sig()?;
            if key == "uris" && signature == b"as" {
                r.array(4, |r| {
                    if let Some(bytes) = crate::dialog::local_file_uri(r.str()?) {
                        paths.push(PathBuf::from(std::ffi::OsString::from_vec(bytes)));
                    }
                    Some(())
                })?;
            } else {
                r.skip(signature, 0)?;
            }
            Some(())
        })
        .ok_or_else(|| anyhow!("invalid chooser result dictionary"))?;
    Ok(paths)
}
pub(crate) fn alert(title: &str, message: &str) {
    log::error!("{title}: {message}");
}
