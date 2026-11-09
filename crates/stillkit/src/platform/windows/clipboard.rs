//! The system clipboard's text (`CF_UNICODETEXT`), for copying and pasting outside fields
//! (a "Copy path" action). Fields type through RichEdit, which uses it directly.
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

const CF_UNICODETEXT: u32 = 13;

/// The system clipboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct System;

impl crate::el::Clipboard for System {
    fn get(&mut self) -> Option<String> {
        // SAFETY: the clipboard is opened and closed around a read of its own handle's memory,
        // copied out before unlocking.
        unsafe {
            OpenClipboard(None).ok()?;
            let text = GetClipboardData(CF_UNICODETEXT).ok().and_then(|h| {
                let global = HGLOBAL(h.0);
                let ptr = GlobalLock(global) as *const u16;
                if ptr.is_null() {
                    return None;
                }
                let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                let _ = GlobalUnlock(global);
                Some(text)
            });
            let _ = CloseClipboard();
            text
        }
    }

    fn set(&mut self, text: &str) {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a fresh movable allocation sized for the text and its terminator, filled while
        // locked; on success the clipboard owns it.
        unsafe {
            let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) else { return };
            let ptr = GlobalLock(global) as *mut u16;
            if ptr.is_null() {
                return;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(global);
            if OpenClipboard(None).is_ok() {
                let _ = EmptyClipboard();
                let _ = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(global.0)));
                let _ = CloseClipboard();
            }
        }
    }
}
