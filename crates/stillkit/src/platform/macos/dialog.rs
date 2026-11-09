//! `NSOpenPanel`, `NSSavePanel` and `NSAlert`, run modally on the main thread.
use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSModalResponseOK, NSOpenPanel, NSSavePanel};
use objc2_foundation::{NSString, NSURL};

use crate::dialog::Request;

pub(crate) fn pick(title: &str, start: Option<&Path>, request: Request<'_>) -> Vec<PathBuf> {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!("file dialogs need the main thread");
        return Vec::new();
    };
    // Bring the app forward so the panel isn't opened behind another app's window.
    objc2_app_kit::NSApplication::sharedApplication(mtm).activate();
    let start = start.map(|p| NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy())));
    let path = |url: &NSURL| url.path().map(|p| PathBuf::from(p.to_string()));
    if let Request::Save { name } = request {
        let panel = NSSavePanel::savePanel(mtm);
        panel.setTitle(Some(&NSString::from_str(title)));
        panel.setMessage(Some(&NSString::from_str(title)));
        panel.setNameFieldStringValue(&NSString::from_str(name));
        panel.setDirectoryURL(start.as_deref());
        if panel.runModal() != NSModalResponseOK {
            return Vec::new();
        }
        return panel.URL().as_deref().and_then(path).into_iter().collect();
    }
    let panel = NSOpenPanel::openPanel(mtm);
    let folder = request == Request::Folder;
    panel.setCanChooseDirectories(folder);
    panel.setCanChooseFiles(!folder);
    panel.setCanCreateDirectories(folder);
    panel.setAllowsMultipleSelection(matches!(request, Request::Files { multiple: true }));
    panel.setTitle(Some(&NSString::from_str(title)));
    panel.setMessage(Some(&NSString::from_str(title)));
    panel.setDirectoryURL(start.as_deref());
    if panel.runModal() != NSModalResponseOK {
        return Vec::new();
    }
    panel.URLs().iter().filter_map(|url| path(&url)).collect()
}

pub(crate) fn alert(title: &str, message: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!("{title}: {message}");
        return;
    };
    objc2_app_kit::NSApplication::sharedApplication(mtm).activate();
    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Critical);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.runModal();
}
