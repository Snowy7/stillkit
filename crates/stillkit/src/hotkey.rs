//! Opt-in global hotkeys; Linux requires a compositor implementing GlobalShortcuts.
use anyhow::{Result, bail, ensure};
/// A global key chord, parsed from `CTRL+ALT+K`, `SUPER+SPACE` or `F1`..`F24`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
pub struct Chord {
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) modifiers: u32,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) key: u32,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) trigger: String,
}
impl std::str::FromStr for Chord {
    type Err = anyhow::Error;
    fn from_str(text: &str) -> Result<Self> {
        let mut modifiers = 0;
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            let part = part.to_ascii_uppercase();
            let modifier = match part.as_str() {
                "CTRL" | "CONTROL" => Some((2, "CTRL")),
                "ALT" => Some((1, "ALT")),
                "SHIFT" => Some((4, "SHIFT")),
                "SUPER" | "WIN" | "META" => Some((8, "LOGO")),
                _ => None,
            };
            if let Some((bit, _)) = modifier {
                ensure!(modifiers & bit == 0, "repeated modifier");
                modifiers |= bit;
            } else {
                ensure!(key.is_none(), "a chord needs exactly one key");
                let value = match part.as_str() {
                    "SPACE" => 32,
                    s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
                        u32::from(s.as_bytes()[0])
                    }
                    s if s.starts_with('F') => {
                        let n = s[1..].parse::<u32>()?;
                        ensure!((1..=24).contains(&n), "invalid function key");
                        0x6f + n
                    }
                    _ => bail!("unsupported hotkey key"),
                };
                key = Some(value);
            }
        }
        let key = key.ok_or_else(|| anyhow::anyhow!("missing hotkey key"))?;
        ensure!(
            modifiers != 0 || (0x70..=0x87).contains(&key),
            "use a modifier with this key"
        );
        let mut names = Vec::new();
        for (bit, name) in [(2, "CTRL"), (1, "ALT"), (4, "SHIFT"), (8, "LOGO")] {
            if modifiers & bit != 0 {
                names.push(name.to_owned());
            }
        }
        let key_name = match key {
            32 => "space".to_owned(),
            0x70..=0x87 => format!("F{}", key - 0x6f),
            _ => char::from_u32(key).unwrap().to_string(),
        };
        names.push(key_name);
        Ok(Self {
            modifiers,
            key,
            trigger: names.join("+"),
        })
    }
}
/// Owns a registration on the UI thread; drop unregisters it.
pub struct Hotkey(crate::platform::current::services::Hotkey);
impl Hotkey {
    /// Registers a chord. Linux may show a compositor consent dialog and choose another trigger.
    pub fn new(chord: Chord, description: &str) -> Result<Self> {
        crate::platform::current::services::Hotkey::new(chord, description).map(Self)
    }
    /// Takes one activation after native event dispatch.
    pub fn activated(&mut self) -> bool {
        self.0.activated()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chords_reject_ambiguous_or_unmodified_typing() {
        assert_eq!("Ctrl+Alt+k".parse::<Chord>().unwrap().modifiers, 3);
        assert_eq!("F24".parse::<Chord>().unwrap().key, 0x87);
        for s in ["K", "CTRL", "CTRL+CTRL+K", "CTRL+K+L", "F25", "CTRL++"] {
            assert!(s.parse::<Chord>().is_err(), "{s}");
        }
    }
}
