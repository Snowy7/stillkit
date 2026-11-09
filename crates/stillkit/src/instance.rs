//! Opt-in single-instance ownership and launch/URL activation delivery.
use anyhow::{Result, ensure};
use std::ffi::OsString;
/// The first launch owns this guard. Later launches forward arguments and return `None`;
/// callers then exit normally. Activation URLs are delivered as ordinary arguments.
pub struct Instance(crate::platform::current::services::Instance);
impl Instance {
    /// Claims a stable app id or forwards `args` to its current owner. Call before making UI.
    pub fn claim(id: &str, args: Vec<OsString>) -> Result<Option<Self>> {
        ensure!(
            !id.is_empty()
                && id.len() <= 160
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
            "invalid instance id"
        );
        let payload = encode(&args)?;
        crate::platform::current::services::Instance::claim(id, payload).map(|i| i.map(Self))
    }
    /// Takes arguments from a later launch, including any activation URL.
    pub fn next_args(&mut self) -> Option<Vec<OsString>> {
        self.0.next_args().and_then(|b| decode(&b))
    }
}
pub(crate) fn encode(args: &[OsString]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(&(args.len() as u32).to_le_bytes());
    for arg in args {
        #[cfg(windows)]
        let bytes: Vec<u8> = {
            use std::os::windows::ffi::OsStrExt;
            let units: Vec<u16> = arg.encode_wide().collect();
            ensure!(!units.contains(&0), "argument contains NUL");
            units.into_iter().flat_map(u16::to_le_bytes).collect()
        };
        #[cfg(unix)]
        let bytes: Vec<u8> = {
            let bytes = arg.as_encoded_bytes();
            ensure!(!bytes.contains(&0), "argument contains NUL");
            bytes.to_vec()
        };
        #[cfg(not(any(windows, unix)))]
        let bytes = {
            let bytes = arg.as_encoded_bytes();
            ensure!(!bytes.contains(&0), "argument contains NUL");
            bytes.to_vec()
        };
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
    }
    ensure!(out.len() <= 65536, "activation arguments exceed 64 KiB");
    Ok(out)
}
pub(crate) fn decode(mut bytes: &[u8]) -> Option<Vec<OsString>> {
    fn word(bytes: &mut &[u8]) -> Option<usize> {
        let n = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
        *bytes = &bytes[4..];
        Some(n)
    }
    if bytes.len() > 65536 {
        return None;
    }
    let count = word(&mut bytes)?;
    if count > 16384 {
        return None;
    }
    let mut args = Vec::new();
    for _ in 0..count {
        let len = word(&mut bytes)?;
        let raw = bytes.get(..len)?;
        #[cfg(windows)]
        let arg = {
            use std::os::windows::ffi::OsStringExt;
            if !raw.len().is_multiple_of(2) {
                return None;
            }
            let units: Vec<u16> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes(*b))
                .collect();
            if units.contains(&0) {
                return None;
            }
            OsString::from_wide(&units)
        };
        #[cfg(unix)]
        let arg = {
            use std::os::unix::ffi::OsStringExt;
            if raw.contains(&0) {
                return None;
            }
            OsString::from_vec(raw.to_vec())
        };
        #[cfg(not(any(windows, unix)))]
        let arg = OsString::from(std::str::from_utf8(raw).ok()?);
        args.push(arg);
        bytes = &bytes[len..];
    }
    bytes.is_empty().then_some(args)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_preserve_empty_unicode_and_activation_urls() {
        let args = vec![
            "".into(),
            "stillkit://open/a%20b".into(),
            "\u{65e5}\u{672c}\u{8a9e}".into(),
        ];
        let bytes = encode(&args).unwrap();
        assert_eq!(decode(&bytes), Some(args));
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n]).is_none());
        }
        assert!(decode(&[255; 4]).is_none());
        assert!(encode(&["\0".into()]).is_err());
        assert!(encode(&["a".repeat(65536).into()]).is_err());
    }
}

#[cfg(all(test, windows))]
mod native_encoding_tests {
    use super::*;
    #[test]
    fn unpaired_utf16_arguments_round_trip() {
        use std::os::windows::ffi::OsStringExt;
        let args = vec![OsString::from_wide(&[b'a' as u16, 0xd800])];
        assert_eq!(decode(&encode(&args).unwrap()), Some(args));
    }
}
#[cfg(all(test, unix))]
mod native_encoding_tests {
    use super::*;
    #[test]
    fn non_utf8_arguments_round_trip() {
        use std::os::unix::ffi::OsStringExt;
        let args = vec![OsString::from_vec(vec![b'a', 255])];
        assert_eq!(decode(&encode(&args).unwrap()), Some(args));
    }
}
