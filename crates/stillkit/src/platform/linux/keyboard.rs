//! Owns an XKB state loaded from the compositor's keymap (no build-time system SDK), and the
//! locale's compose table (dead keys, Compose sequences) when libxkbcommon has one.
use xkbcommon_dl as xkb;
pub struct Keyboard {
    api: &'static xkb::XkbCommon,
    state: std::ptr::NonNull<xkb::xkb_state>,
    compose: Option<Compose>,
}

/// A compose state for the user's locale.
struct Compose {
    api: &'static xkb::XkbCommonCompose,
    table: std::ptr::NonNull<xkb::xkb_compose_table>,
    state: std::ptr::NonNull<xkb::xkb_compose_state>,
}

/// What a key press types once compose has seen it.
pub enum Typed {
    /// The key's own text (empty for keys that type nothing).
    Text(String),
    /// Part of a sequence (a dead key): nothing yet.
    Composing,
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
                compose: Compose::new(),
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
    /// The modifier keys held (Control, Alt as Mod1, Super as Mod4, Shift).
    pub fn modifiers_held(&self) -> crate::el::Modifiers {
        let on = |name: &std::ffi::CStr| {
            // SAFETY: the state is live and the modifier name is a static NUL-terminated string.
            unsafe {
                (self.api.xkb_state_mod_name_is_active)(
                    self.state.as_ptr(),
                    name.as_ptr(),
                    xkb::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
                ) > 0
            }
        };
        crate::el::Modifiers { shift: on(c"Shift"), ctrl: on(c"Control"), alt: on(c"Mod1"), meta: on(c"Mod4") }
    }
    /// What pressing `key` types, through the compose table: a dead key types nothing until
    /// its sequence ends, then the composed text.
    pub fn typed(&mut self, key: u32) -> Typed {
        let sym = self.symbol(key);
        let Some(c) = &self.compose else { return Typed::Text(self.text(key)) };
        // SAFETY: the compose state is live and owned by this keyboard.
        unsafe {
            (c.api.xkb_compose_state_feed)(c.state.as_ptr(), sym);
            match (c.api.xkb_compose_state_get_status)(c.state.as_ptr()) {
                xkb::xkb_compose_status::XKB_COMPOSE_COMPOSING => Typed::Composing,
                xkb::xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                    let mut bytes = [0u8; 64];
                    let n = (c.api.xkb_compose_state_get_utf8)(c.state.as_ptr(), bytes.as_mut_ptr().cast(), bytes.len());
                    (c.api.xkb_compose_state_reset)(c.state.as_ptr());
                    let n = (n.max(0) as usize).min(bytes.len() - 1);
                    Typed::Text(String::from_utf8_lossy(&bytes[..n]).into_owned())
                }
                xkb::xkb_compose_status::XKB_COMPOSE_CANCELLED => {
                    (c.api.xkb_compose_state_reset)(c.state.as_ptr());
                    Typed::Text(String::new())
                }
                _ => Typed::Text(self.text(key)),
            }
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
impl Compose {
    fn new() -> Option<Self> {
        let api = xkb::xkbcommon_compose_option()?;
        let base = xkb::xkbcommon_option()?;
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|l| !l.is_empty()))
            .unwrap_or_else(|| "C".into());
        let locale = std::ffi::CString::new(locale).ok()?;
        // SAFETY: a fresh context and table from documented constructors, checked for null;
        // the context reference is released once the table holds its own.
        unsafe {
            let context = (base.xkb_context_new)(xkb::xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return None;
            }
            let table = (api.xkb_compose_table_new_from_locale)(
                context,
                locale.as_ptr(),
                xkb::xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
            );
            (base.xkb_context_unref)(context);
            let table = std::ptr::NonNull::new(table)?;
            let state = (api.xkb_compose_state_new)(table.as_ptr(), xkb::xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS);
            let Some(state) = std::ptr::NonNull::new(state) else {
                (api.xkb_compose_table_unref)(table.as_ptr());
                return None;
            };
            Some(Self { api, table, state })
        }
    }
}
impl Drop for Compose {
    fn drop(&mut self) {
        // SAFETY: the final owner of both references.
        unsafe {
            (self.api.xkb_compose_state_unref)(self.state.as_ptr());
            (self.api.xkb_compose_table_unref)(self.table.as_ptr());
        }
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
