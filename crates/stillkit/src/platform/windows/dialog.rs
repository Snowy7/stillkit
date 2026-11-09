//! `IFileOpenDialog`, `IFileSaveDialog` and `MessageBoxW`, modal on the calling thread.
use std::path::{Path, PathBuf};

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileOpenDialog,
    IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::HSTRING;

use crate::dialog::Request;

pub(crate) fn pick(title: &str, start: Option<&Path>, request: Request<'_>) -> Vec<PathBuf> {
    // SAFETY: COM calls on this thread with owned interface pointers; strings outlive the calls.
    unsafe { show(title, start, request) }.unwrap_or_else(|e| {
        // Cancelling is an error HRESULT too (ERROR_CANCELLED): nothing to report.
        log::debug!("file dialog: {e}");
        Vec::new()
    })
}

unsafe fn show(title: &str, start: Option<&Path>, request: Request<'_>) -> windows::core::Result<Vec<PathBuf>> {
    // SAFETY: (all below) the caller's contract; apartment init is idempotent per thread.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let owner = GetActiveWindow();
        let owner = (!owner.is_invalid()).then_some(owner);
        let folder = start.and_then(|p| SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(p), None).ok());
        let path = |item: &IShellItem| -> windows::core::Result<PathBuf> {
            let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = PathBuf::from(name.to_string().unwrap_or_default());
            CoTaskMemFree(Some(name.0.cast()));
            Ok(path)
        };
        if let Request::Save { name } = request {
            let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetTitle(&HSTRING::from(title))?;
            dialog.SetFileName(&HSTRING::from(name))?;
            if let Some(folder) = &folder {
                dialog.SetFolder(folder)?;
            }
            dialog.Show(owner)?;
            return Ok(vec![path(&dialog.GetResult()?)?]);
        }
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let mut options = dialog.GetOptions()? | FOS_FORCEFILESYSTEM;
        match request {
            Request::Folder => options |= FOS_PICKFOLDERS,
            Request::Files { multiple: true } => options |= FOS_ALLOWMULTISELECT,
            _ => {}
        }
        dialog.SetOptions(options)?;
        dialog.SetTitle(&HSTRING::from(title))?;
        if let Some(folder) = &folder {
            dialog.SetFolder(folder)?;
        }
        dialog.Show(owner)?;
        let items = dialog.GetResults()?;
        (0..items.GetCount()?).map(|i| path(&items.GetItemAt(i)?)).collect()
    }
}

pub(crate) fn alert(title: &str, message: &str) {
    // SAFETY: a message box with owned, nul-terminated strings that outlive the call.
    unsafe {
        let owner = GetActiveWindow();
        MessageBoxW(Some(owner), &HSTRING::from(message), &HSTRING::from(title), MB_OK | MB_ICONERROR);
    }
}
