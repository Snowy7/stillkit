//! What this process's windows are called, so other processes and tools can find them.
//!
//! Surfaces register the window class `<prefix>`, app windows `<prefix>.app`, and desktop-band
//! surfaces carry the window property `<prefix>.desktop`. The prefix is `stillkit` unless the app
//! calls [`set_identity`] before its first window; an app whose other processes look its windows
//! up by class (`FindWindowW`) sets the same prefix in each of them.

use std::sync::OnceLock;

use windows::core::PCWSTR;

/// An app's names for its windows. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// The window class prefix, e.g. `"stillkit"`.
    pub prefix: &'static str,
    /// Window classes of other windows that live in the desktop band beside this app's
    /// desktop surfaces (another process's backdrop, say). Desktop surfaces keep their order
    /// relative to these as to their own.
    pub desktop_classes: &'static [&'static str],
}

impl Default for Identity {
    fn default() -> Self {
        Self { prefix: "stillkit", desktop_classes: &[] }
    }
}

struct Names {
    identity: Identity,
    surface: Vec<u16>,
    app: Vec<u16>,
    desktop: Vec<u16>,
}

static NAMES: OnceLock<Names> = OnceLock::new();

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn names_of(identity: Identity) -> Names {
    let prefix = identity.prefix;
    Names {
        identity,
        surface: wide(prefix),
        app: wide(&format!("{prefix}.app")),
        desktop: wide(&format!("{prefix}.desktop")),
    }
}

fn names() -> &'static Names {
    NAMES.get_or_init(|| names_of(Identity::default()))
}

/// Sets the names for this process's windows. Call it once, before the first window; it fails
/// (returning the identity in use) when the names were already set or a window already used them.
pub fn set_identity(identity: Identity) -> Result<(), Identity> {
    let mut fresh = false;
    let names = NAMES.get_or_init(|| {
        fresh = true;
        names_of(identity)
    });
    if fresh || names.identity == identity { Ok(()) } else { Err(names.identity) }
}

/// The class of [`Surface`](crate::Surface) windows.
pub(crate) fn surface_class() -> PCWSTR {
    PCWSTR(names().surface.as_ptr())
}

/// The class of app windows ([`Kind::App`](crate::Kind::App)).
pub(crate) fn app_class() -> PCWSTR {
    PCWSTR(names().app.as_ptr())
}

/// The property that marks a desktop-band surface.
pub(crate) fn desktop_prop() -> PCWSTR {
    PCWSTR(names().desktop.as_ptr())
}

/// Whether `class` (a window's class name) is one of the identity's desktop classes.
pub(crate) fn is_desktop_class(class: &[u16]) -> bool {
    names()
        .identity
        .desktop_classes
        .iter()
        .any(|name| class.iter().copied().eq(name.encode_utf16()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_prefix() {
        let names = names_of(Identity { prefix: "acme.ui", desktop_classes: &[] });
        assert_eq!(names.surface, wide("acme.ui"));
        assert_eq!(names.app, wide("acme.ui.app"));
        assert_eq!(names.desktop, wide("acme.ui.desktop"));
    }

    #[test]
    fn a_second_different_identity_is_refused() {
        let first = Identity { prefix: "acme.ui", desktop_classes: &["acme.glass"] };
        // The process-wide names may already hold the default (another test made a window).
        let set = set_identity(first);
        let current = set.err().unwrap_or(first);
        assert_eq!(set_identity(current), Ok(()), "the same identity again is fine");
        let other = Identity { prefix: "other", desktop_classes: &[] };
        assert_eq!(set_identity(other), Err(current));
    }
}
