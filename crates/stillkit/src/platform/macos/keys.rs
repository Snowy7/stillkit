//! macOS key codes (`kVK_*`, the physical key) to the virtual-key codes `Event::Key` carries.
//! Letters and digits come from the layout's own character, so Command-Z is Z on AZERTY too.

/// The virtual key for a key-down, or `None` for keys that only type characters.
pub(super) fn vk(code: u16, unmodified: Option<char>) -> Option<u16> {
    Some(match code {
        0x7B => 0x25,        // Left
        0x7C => 0x27,        // Right
        0x7D => 0x28,        // Down
        0x7E => 0x26,        // Up
        0x35 => 0x1b,        // Escape
        0x24 | 0x4C => 0x0d, // Return, keypad Enter
        0x30 => 0x09,        // Tab
        0x33 => 0x08,        // Delete (backspace)
        0x75 => 0x2e,        // Forward delete
        0x73 => 0x24,        // Home
        0x77 => 0x23,        // End
        0x74 => 0x21,        // Page Up
        0x79 => 0x22,        // Page Down
        0x72 => 0x2d,        // Help, where Insert sits on PC keyboards
        0x6E => 0x5d,        // Context menu
        0x31 => 0x20,        // Space
        0x7A => 0x70,        // F1..F12
        0x78 => 0x71,
        0x63 => 0x72,
        0x76 => 0x73,
        0x60 => 0x74,
        0x61 => 0x75,
        0x62 => 0x76,
        0x64 => 0x77,
        0x65 => 0x78,
        0x6D => 0x79,
        0x67 => 0x7a,
        0x6F => 0x7b,
        _ => match unmodified? {
            c @ ('a'..='z' | 'A'..='Z' | '0'..='9') => c.to_ascii_uppercase() as u16,
            _ => return None,
        },
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn arrows_editing_keys_and_letters_map_to_virtual_keys() {
        use super::vk;
        assert_eq!(vk(0x7B, None), Some(0x25));
        assert_eq!(vk(0x33, Some('\u{7f}')), Some(0x08));
        assert_eq!(vk(0x00, Some('a')), Some(b'A' as u16));
        assert_eq!(vk(0x19, Some('9')), Some(b'9' as u16));
        assert_eq!(vk(0x2B, Some(',')), None, "punctuation only types");
    }
}
