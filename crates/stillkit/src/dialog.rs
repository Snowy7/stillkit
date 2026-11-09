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
//! `IFileSaveDialog`, `MessageBoxW`. Linux has no portal client yet: pickers answer nothing and
//! alerts go to the log.

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
    native::pick(title, start, Request::Folder).into_iter().next()
}

/// Asks for existing files (several when `multiple`). Empty when cancelled.
pub fn pick_files(title: &str, start: Option<&Path>, multiple: bool) -> Vec<PathBuf> {
    native::pick(title, start, Request::Files { multiple })
}

/// Asks where to save, suggesting `name` in `start`. `None` when cancelled.
pub fn save_file(title: &str, start: Option<&Path>, name: &str) -> Option<PathBuf> {
    native::pick(title, start, Request::Save { name }).into_iter().next()
}

/// Shows an error message box with one OK button, for failures with no window to show them
/// in (start-up).
pub fn alert(title: &str, message: &str) {
    native::alert(title, message);
}
