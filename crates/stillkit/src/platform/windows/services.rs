//! Opt-in shell services. New entry points are resolved only when a service is created.
use crate::tray::{Event, Icon};
use anyhow::{Result, anyhow, ensure};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
            },
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, s, w},
};
type DestroyIconFn = unsafe extern "system" fn(HICON) -> i32;
type CursorPosFn = unsafe extern "system" fn(*mut windows::Win32::Foundation::POINT) -> i32;
type Notify = unsafe extern "system" fn(u32, *const NOTIFYICONDATAW) -> i32;
type RegisterMessage = unsafe extern "system" fn(PCWSTR) -> u32;
type CreateIcon = unsafe extern "system" fn(*const u8, u32, i32, u32, i32, i32, u32) -> HICON;
/// Looks up a system DLL procedure; the module remains mapped for its function pointers.
type RawProcedure = unsafe extern "system" fn() -> isize;
fn procedure(dll: PCWSTR, name: windows::core::PCSTR) -> Result<RawProcedure> {
    // SAFETY: fixed system DLL names, system-only search, valid terminated procedure name.
    unsafe {
        let module = LoadLibraryExW(dll, None, LOAD_LIBRARY_SEARCH_SYSTEM32)?;
        GetProcAddress(module, name).ok_or_else(|| anyhow!("desktop service API unavailable"))
    }
}
struct State {
    data: NOTIFYICONDATAW,
    notify: Notify,
    destroy: DestroyIconFn,
    cursor: CursorPosFn,
    taskbar: u32,
    notification: u32,
    clicked: VecDeque<u32>,
    events: Rc<RefCell<VecDeque<Event>>>,
}
pub(crate) struct Tray {
    window: HWND,
    state: Box<State>,
}
impl Tray {
    pub fn new(_: &str, tooltip: &str, icon: Icon) -> Result<Self> {
        // SAFETY: each symbol has its documented ABI; loaded DLLs remain mapped.
        let (notify, register, create, destroy, cursor): (
            Notify,
            RegisterMessage,
            CreateIcon,
            DestroyIconFn,
            CursorPosFn,
        ) = unsafe {
            (
                std::mem::transmute::<RawProcedure, Notify>(procedure(
                    w!("shell32.dll"),
                    s!("Shell_NotifyIconW"),
                )?),
                std::mem::transmute::<RawProcedure, RegisterMessage>(procedure(
                    w!("user32.dll"),
                    s!("RegisterWindowMessageW"),
                )?),
                std::mem::transmute::<RawProcedure, CreateIcon>(procedure(
                    w!("user32.dll"),
                    s!("CreateIconFromResourceEx"),
                )?),
                std::mem::transmute::<RawProcedure, DestroyIconFn>(procedure(
                    w!("user32.dll"),
                    s!("DestroyIcon"),
                )?),
                std::mem::transmute::<RawProcedure, CursorPosFn>(procedure(
                    w!("user32.dll"),
                    s!("GetCursorPos"),
                )?),
            )
        };
        let dib = icon_dib(&icon);
        // SAFETY: dib contains a complete icon DIB with color and mask planes.
        let handle = unsafe {
            create(
                dib.as_ptr(),
                dib.len() as u32,
                1,
                0x30000,
                icon.width as i32,
                icon.height as i32,
                0,
            )
        };
        ensure!(!handle.0.is_null(), "could not create tray icon");
        let mut state = Box::new(State {
            data: NOTIFYICONDATAW::default(),
            notify,
            destroy,
            cursor,
            // SAFETY: terminated constant message name.
            taskbar: unsafe { register(w!("TaskbarCreated")) },
            events: Rc::default(),
            notification: 0,
            clicked: VecDeque::new(),
        });
        let class = w!("stillkit.services.tray.v1");
        // SAFETY: the hidden top-level window receives Explorer broadcasts; it is never shown.
        let window = unsafe {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
        };
        let window = match window {
            Ok(w) => w,
            Err(e) => {
                // SAFETY: handle is a newly created icon owned here.
                unsafe {
                    let _ = destroy(handle);
                }
                return Err(e.into());
            }
        };
        state.data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        state.data.hWnd = window;
        state.data.uID = 1;
        state.data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        state.data.uCallbackMessage = WM_APP + 37;
        state.data.hIcon = handle;
        copy_wide(&mut state.data.szTip, tooltip);
        // SAFETY: the boxed state is stable until the window is destroyed in Drop.
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, (&raw mut *state) as isize);
        }
        let tray = Self { window, state };
        ensure!(tray.state.taskbar != 0, "could not register TaskbarCreated");
        // SAFETY: data is initialized and its window and icon remain owned by tray.
        ensure!(
            // SAFETY: initialized icon data and its owned window remain live.
            unsafe { (tray.state.notify)(NIM_ADD.0, &tray.state.data) } != 0,
            "Shell_NotifyIconW failed"
        );
        Ok(tray)
    }
    pub fn next_event(&mut self) -> Option<Event> {
        self.state.events.borrow_mut().pop_front()
    }
    pub fn set_tooltip(&mut self, text: &str) -> Result<()> {
        copy_wide(&mut self.state.data.szTip, text);
        // SAFETY: this icon and its window remain live.
        ensure!(
            // SAFETY: initialized icon data and its owned window remain live.
            unsafe { (self.state.notify)(NIM_MODIFY.0, &self.state.data) } != 0,
            "tray tooltip update failed"
        );
        Ok(())
    }
}
/// Builds the color and AND-mask planes without allocating a native icon.
fn icon_dib(icon: &Icon) -> Vec<u8> {
    let mut dib = Vec::new();
    for value in [40u32, icon.width, icon.height * 2] {
        dib.extend_from_slice(&value.to_le_bytes());
    }
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&[0; 24]);
    for row in icon.pixels.chunks_exact(icon.width as usize * 4).rev() {
        for p in row.as_chunks::<4>().0 {
            let a = u16::from(p[3]);
            dib.extend_from_slice(&[
                (u16::from(p[2]) * a / 255) as u8,
                (u16::from(p[1]) * a / 255) as u8,
                (u16::from(p[0]) * a / 255) as u8,
                p[3],
            ]);
        }
    }
    dib.resize(
        dib.len() + icon.width.div_ceil(32) as usize * 4 * icon.height as usize,
        0,
    );
    let stride = icon.width.div_ceil(32) as usize * 4;
    let mask_at = 40 + icon.width as usize * icon.height as usize * 4;
    for (y, row) in icon
        .pixels
        .chunks_exact(icon.width as usize * 4)
        .rev()
        .enumerate()
    {
        for (x, pixel) in row.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] == 0 {
                dib[mask_at + y * stride + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    dib
}
fn copy_wide<const N: usize>(out: &mut [u16; N], text: &str) {
    out.fill(0);
    for (slot, unit) in out.iter_mut().take(N - 1).zip(text.encode_utf16()) {
        *slot = unit;
    }
}
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: userdata is installed only for the boxed state owned by Tray.
    let ptr = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut State;
    if !ptr.is_null() {
        // SAFETY: destruction clears the pointer before freeing its box.
        let state = unsafe { &mut *ptr };
        if message == state.taskbar && state.taskbar != 0 {
            state.notification = 0;
            // SAFETY: Explorer restarted; the icon data and window are still live.
            unsafe {
                (state.notify)(NIM_ADD.0, &state.data);
            }
            return LRESULT(0);
        }
        if message == WM_APP + 37 {
            if matches!(lparam.0 as u32, 0x403 | 0x404) {
                state.notification = 0;
                return LRESULT(0);
            }
            if lparam.0 as u32 == 0x405 && state.notification != 0 {
                if state.clicked.len() < 256 {
                    state.clicked.push_back(state.notification);
                    state.notification = 0;
                }
                return LRESULT(0);
            }
            let mut at = windows::Win32::Foundation::POINT::default();
            // SAFETY: at is a writable POINT; this query never moves the cursor.
            unsafe {
                let _ = (state.cursor)(&mut at);
            }
            let event = match lparam.0 as u32 {
                WM_LBUTTONUP => Some(Event::Left(at.x, at.y)),
                WM_RBUTTONUP => Some(Event::Right(at.x, at.y)),
                WM_LBUTTONDBLCLK => Some(Event::Double(at.x, at.y)),
                _ => None,
            };
            if let Some(event) = event {
                let mut events = state.events.borrow_mut();
                if events.len() < 256 {
                    events.push_back(event);
                }
            }
            return LRESULT(0);
        }
    }
    // SAFETY: messages not handled here follow the system's default processing.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: remove the shell reference, clear userdata, then destroy our window and icon.
        unsafe {
            (self.state.notify)(NIM_DELETE.0, &self.state.data);
            SetWindowLongPtrW(self.window, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.window);
            let _ = (self.state.destroy)(self.state.data.hIcon);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_lifetime_never_activates_its_hidden_window() -> Result<()> {
        let mut tray = Tray::new(
            "stillkit.tray.test",
            "stillkit tray lifetime test",
            Icon::rgba(16, 16, vec![255; 16 * 16 * 4])?,
        )?;
        // SAFETY: visibility and foreground queries only.
        unsafe {
            assert!(!IsWindowVisible(tray.window).as_bool());
            assert_ne!(tray.window, GetForegroundWindow());
        }
        // SAFETY: remove our test icon, then invoke our recovery callback without any input.
        unsafe {
            (tray.state.notify)(NIM_DELETE.0, &tray.state.data);
            window_proc(tray.window, tray.state.taskbar, WPARAM(0), LPARAM(0));
        }
        tray.set_tooltip("restored after TaskbarCreated")?;
        let window = tray.window;
        drop(tray);
        // SAFETY: IsWindow accepts handles of destroyed windows.
        unsafe {
            assert!(!IsWindow(Some(window)).as_bool());
        }
        Ok(())
    }
    #[test]
    fn rgba_icon_keeps_transparency_and_premultiplies_color() -> Result<()> {
        let icon = Icon::rgba(2, 1, vec![255, 128, 64, 128, 0, 0, 0, 0])?;
        let dib = icon_dib(&icon);
        assert_eq!(&dib[40..48], &[32, 64, 128, 128, 0, 0, 0, 0]);
        assert_eq!(&dib[48..], &[0x40, 0, 0, 0]);
        let transparent = icon_dib(&Icon::rgba(1, 1, vec![0; 4])?);
        assert_eq!(&transparent[44..], &[0x80, 0, 0, 0]);
        Ok(())
    }
    #[test]
    fn tooltip_is_terminated_and_clears_previous_text() {
        let mut out = [0u16; 4];
        copy_wide(&mut out, "abcde");
        assert_eq!(out, [97, 98, 99, 0]);
        copy_wide(&mut out, "x");
        assert_eq!(out, [120, 0, 0, 0]);
    }
}

type RegisterHotkey = unsafe extern "system" fn(HWND, i32, u32, u32) -> i32;
type UnregisterHotkey = unsafe extern "system" fn(HWND, i32) -> i32;
pub(crate) struct Hotkey {
    window: HWND,
    pending: Box<std::cell::Cell<u32>>,
    unregister: UnregisterHotkey,
}
impl Hotkey {
    pub fn new(chord: crate::hotkey::Chord, _: &str) -> Result<Self> {
        // SAFETY: signatures match RegisterHotKey and UnregisterHotKey; DLLs stay mapped.
        let (register, unregister): (RegisterHotkey, UnregisterHotkey) = unsafe {
            (
                std::mem::transmute::<RawProcedure, RegisterHotkey>(procedure(
                    w!("user32.dll"),
                    s!("RegisterHotKey"),
                )?),
                std::mem::transmute::<RawProcedure, UnregisterHotkey>(procedure(
                    w!("user32.dll"),
                    s!("UnregisterHotKey"),
                )?),
            )
        };
        let class = w!("stillkit.services.hotkey.v1");
        // SAFETY: a message-only window, never shown or activated.
        let window = unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(hotkey_proc),
                lpszClassName: class,
                ..Default::default()
            });
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )?
        };
        let mut hotkey = Self {
            window,
            pending: Box::default(),
            unregister,
        };
        // SAFETY: pending's allocation stays stable until destruction clears userdata.
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, (&raw mut *hotkey.pending) as isize);
        }
        // SAFETY: this window owns id 1; MOD_NOREPEAT prevents autorepeat activations.
        ensure!(
            // SAFETY: the live hidden window owns this registration.
            unsafe { register(window, 1, chord.modifiers | 0x4000, chord.key) } != 0,
            "hotkey registration failed or chord already owned"
        );
        Ok(hotkey)
    }
    pub fn activated(&mut self) -> bool {
        let n = self.pending.get();
        self.pending.set(n.saturating_sub(1));
        n > 0
    }
}
unsafe extern "system" fn hotkey_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_HOTKEY {
        // SAFETY: userdata belongs to the live Hotkey's boxed Cell.
        let ptr =
            unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const std::cell::Cell<u32>;
        if !ptr.is_null() {
            // SAFETY: drop clears userdata before destroying the allocation.
            let pending = unsafe { &*ptr };
            pending.set(pending.get().saturating_add(1));
        }
        return LRESULT(0);
    }
    // SAFETY: default processing for messages not owned here.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
impl Drop for Hotkey {
    fn drop(&mut self) {
        // SAFETY: registration and hidden window are owned here.
        unsafe {
            (self.unregister)(self.window, 1);
            SetWindowLongPtrW(self.window, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.window);
        }
    }
}

#[cfg(test)]
mod hotkey_tests {
    use super::*;
    #[test]
    fn dropping_hotkey_releases_its_chord() -> Result<()> {
        let chord = "CTRL+ALT+SHIFT+F24".parse::<crate::hotkey::Chord>()?;
        let first = Hotkey::new(chord.clone(), "stillkit test")?;
        assert!(Hotkey::new(chord.clone(), "conflicting test").is_err());
        drop(first);
        drop(Hotkey::new(chord, "released test")?);
        Ok(())
    }
}

pub(crate) struct Notifications {
    tray: Tray,
    serial: u32,
}
impl Notifications {
    pub fn new(id: &str, icon: Icon) -> Result<Self> {
        Ok(Self {
            tray: Tray::new(id, id, icon)?,
            serial: 0,
        })
    }
    pub fn show(&mut self, title: &str, body: &str) -> Result<u32> {
        use windows::Win32::UI::Shell::{NIF_INFO, NIIF_INFO};
        ensure!(
            self.tray.state.notification == 0,
            "a notification balloon is still active"
        );
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or_else(|| anyhow!("notification identifier exhausted"))?;
        let data = &mut self.tray.state.data;
        copy_wide(&mut data.szInfoTitle, title);
        copy_wide(&mut data.szInfo, body);
        data.dwInfoFlags = NIIF_INFO;
        let old = data.uFlags;
        data.uFlags = NIF_INFO;
        // SAFETY: the initialized notification data refers to this object's live icon.
        let shown = unsafe { (self.tray.state.notify)(NIM_MODIFY.0, data) } != 0;
        data.uFlags = old;
        ensure!(shown, "notification delivery failed");
        self.tray.state.notification = self.serial;
        Ok(self.serial)
    }
    pub fn next_action(&mut self) -> Option<crate::notify::Action> {
        self.tray
            .state
            .clicked
            .pop_front()
            .map(|id| crate::notify::Action {
                id,
                action: "default".into(),
            })
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    #[test]
    fn balloon_click_delivers_its_identifier_once() -> Result<()> {
        let mut notifications = Notifications::new(
            "stillkit.notification.test",
            Icon::rgba(1, 1, vec![255; 4])?,
        )?;
        notifications.tray.state.notification = 17;
        // SAFETY: directly exercises our callback with its owned window; no input or balloon is sent.
        unsafe {
            window_proc(
                notifications.tray.window,
                WM_APP + 37,
                WPARAM(0),
                LPARAM(0x405),
            );
        }
        assert_eq!(
            notifications.next_action(),
            Some(crate::notify::Action {
                id: 17,
                action: "default".into()
            })
        );
        assert_eq!(notifications.next_action(), None);
        Ok(())
    }
}
