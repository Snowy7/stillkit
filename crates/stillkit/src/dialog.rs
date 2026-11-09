//! Native file dialogs and alerts: the system's own folder and file pickers, save panel and
//! message box, modal to the app. They block the calling (UI) thread until answered, as the
//! platforms' modal loops do; window events that arrive meanwhile wait in the queue.
//!
//! ```ignore
//! if let Some(dir) = stillkit::dialog::pick_folder("Choose a project folder", None) { .. }
//! stillkit::dialog::alert("Couldn't start", &format!("{err:#}"));
//! ```
//!
//! macOS: `NSOpenPanel`, `NSSavePanel`, `NSAlert`. Windows: `IFileOpenDialog`,
//! `IFileSaveDialog`, `MessageBoxW`. Linux: XDG FileChooser portal (requires a desktop
//! portal backend); alerts go to the log. Portal consent waits up to two minutes.

use std::path::{Path, PathBuf};

use crate::platform::current::dialog as native;

/// What a picker asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Request<'a> {
    /// One folder.
    Folder,
    /// Existing files; several when `multiple`.
    Files { multiple: bool },
    /// A new or existing file to write, suggesting `name`.
    Save { name: &'a str },
}

/// Asks for one folder, starting in `start` when given. `None` when cancelled.
pub fn pick_folder(title: &str, start: Option<&Path>) -> Option<PathBuf> {
    native::pick(title, start, Request::Folder)
        .into_iter()
        .next()
}

/// Asks for existing files (several when `multiple`). Empty when cancelled.
pub fn pick_files(title: &str, start: Option<&Path>, multiple: bool) -> Vec<PathBuf> {
    native::pick(title, start, Request::Files { multiple })
}

/// Asks where to save, suggesting `name` in `start`. `None` when cancelled.
pub fn save_file(title: &str, start: Option<&Path>, name: &str) -> Option<PathBuf> {
    native::pick(title, start, Request::Save { name })
        .into_iter()
        .next()
}

/// Shows an error message box with one OK button, for failures with no window to show them
/// in (start-up).
pub fn alert(title: &str, message: &str) {
    native::alert(title, message);
}

/// Converts local file URIs to raw Unix path bytes, preserving non-UTF-8 filenames.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn local_file_uri(uri: &str) -> Option<Vec<u8>> {
    let rest = uri.strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        rest.strip_prefix("localhost")
            .filter(|p| p.starts_with('/'))?
    };
    if path.contains(['?', '#']) {
        return None;
    }
    let mut bytes = Vec::new();
    let mut input = path.bytes();
    while let Some(byte) = input.next() {
        let byte = if byte == b'%' {
            let pair = [input.next()?, input.next()?];
            u8::from_str_radix(std::str::from_utf8(&pair).ok()?, 16).ok()?
        } else {
            byte
        };
        if byte == 0 {
            return None;
        }
        bytes.push(byte);
    }
    Some(bytes)
}
#[cfg(test)]
mod portal_uri_tests {
    use super::*;
    #[test]
    fn only_local_file_uris_become_paths() {
        assert_eq!(
            local_file_uri("file:///tmp/a%20b+%FF"),
            Some(b"/tmp/a b+\xff".to_vec())
        );
        assert_eq!(
            local_file_uri("file://localhost/tmp/a"),
            Some(b"/tmp/a".to_vec())
        );
        for uri in [
            "https://example.com/a",
            "file://server/a",
            "file:///a%00b",
            "file:///a%2",
            "file:///a#fragment",
            "file:///a?query",
        ] {
            assert!(local_file_uri(uri).is_none(), "{uri}");
        }
    }
}
