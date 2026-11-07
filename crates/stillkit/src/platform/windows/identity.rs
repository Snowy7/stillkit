//! What this process's windows are called, so other processes and tools can find them.
//!
//! Surfaces register the window class `<prefix>`, app windows `<prefix>.app`, and desktop-band
//! surfaces carry the window property `<prefix>.desktop`. The prefix is `stillkit` unless the app
//! calls [`set_identity`] before its first window; an app whose other processes look its windows
//! up by class (`FindWindowW`) sets the same prefix in each of them.

use std::sync::OnceLock;
pub use crate::identity::{Identity, set_identity};

use windows::core::PCWSTR;

struct Names {
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
        surface: wide(prefix),
        app: wide(&format!("{prefix}.app")),
        desktop: wide(&format!("{prefix}.desktop")),
    }
}

fn names() -> &'static Names {
    NAMES.get_or_init(|| names_of(*crate::identity::current()))
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
    crate::identity::current()
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
