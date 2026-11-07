//! Copying and pasting text through the general NSPasteboard, for `el`'s text fields:
//! [`System`] is the [`crate::el::Clipboard`] `el::run` gives a runtime on macOS.
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

/// The system clipboard (the general pasteboard), plain text only.
pub struct System;

impl crate::el::Clipboard for System {
    fn get(&mut self) -> Option<String> {
        // SAFETY: an extern static AppKit defines.
        let kind = unsafe { NSPasteboardTypeString };
        NSPasteboard::generalPasteboard().stringForType(kind).map(|s| s.to_string())
    }

    fn set(&mut self, text: &str) {
        // SAFETY: as above.
        let kind = unsafe { NSPasteboardTypeString };
        let board = NSPasteboard::generalPasteboard();
        board.clearContents();
        board.setString_forType(&NSString::from_str(text), kind);
    }
}
