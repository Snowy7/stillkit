//! No portal client yet: pickers answer nothing; alerts go to the log.
use std::path::{Path, PathBuf};

pub(crate) fn pick(title: &str, _: Option<&Path>, _: crate::dialog::Request<'_>) -> Vec<PathBuf> {
    log::warn!("{title}: no native file dialog on this platform yet");
    Vec::new()
}

pub(crate) fn alert(title: &str, message: &str) {
    log::error!("{title}: {message}");
}
