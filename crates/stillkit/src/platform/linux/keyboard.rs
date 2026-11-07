//! Owns an XKB state loaded from the compositor's keymap (no build-time system SDK).
use xkbcommon_dl as xkb;
pub struct Keyboard {
    api: &'static xkb::XkbCommon,
    state: std::ptr::NonNull<xkb::xkb_state>,
}
impl Keyboard {
    pub fn new(fd: std::os::fd::OwnedFd, size: usize) -> Option<Self> {
        use std::io::Read;
        let api = xkb::xkbcommon_option()?;
        let mut bytes = Vec::new();
        std::fs::File::from(fd)
            .take(size as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        let string = std::ffi::CStr::from_bytes_until_nul(&bytes).ok()?;
        // SAFETY: the keymap is NUL-terminated, pointers are checked, and each temporary
        // reference is released after xkb_state_new takes its own keymap reference.
        unsafe {
            let context = (api.xkb_context_new)(xkb::xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return None;
            }
            let map = (api.xkb_keymap_new_from_string)(
                context,
                string.as_ptr(),
                xkb::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            (api.xkb_context_unref)(context);
            if map.is_null() {
                return None;
            }
            let state = (api.xkb_state_new)(map);
            (api.xkb_keymap_unref)(map);
            Some(Self {
                api,
                state: std::ptr::NonNull::new(state)?,
            })
        }
    }
    pub fn modifiers(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        // SAFETY: this object exclusively owns the live XKB state.
        unsafe {
            (self.api.xkb_state_update_mask)(
                self.state.as_ptr(),
                depressed,
                latched,
                locked,
                0,
                0,
                group,
            );
        }
    }
    pub fn shift_down(&self) -> bool {
        // SAFETY: the state is live and the modifier name is a static NUL-terminated string.
        unsafe {
            (self.api.xkb_state_mod_name_is_active)(
                self.state.as_ptr(),
                c"Shift".as_ptr(),
                xkb::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
            ) > 0
        }
    }
    pub fn text(&self, key: u32) -> String {
        let mut bytes = [0u8; 128];
        // SAFETY: a live state and a writable buffer of the stated size are provided.
        let n = unsafe {
            (self.api.xkb_state_key_get_utf8)(
                self.state.as_ptr(),
                key + 8,
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        };
        if n <= 0 || n as usize >= bytes.len() {
            return String::new();
        }
        String::from_utf8_lossy(&bytes[..n as usize]).into_owned()
    }
    pub fn symbol(&self, key: u32) -> u32 {
        // SAFETY: this object owns the live state; Wayland keycodes are evdev (+8 for XKB).
        unsafe { (self.api.xkb_state_key_get_one_sym)(self.state.as_ptr(), key + 8) }
    }
}
impl Drop for Keyboard {
    fn drop(&mut self) {
        // SAFETY: this is the final owner of this state reference.
        unsafe {
            (self.api.xkb_state_unref)(self.state.as_ptr());
        }
    }
}
