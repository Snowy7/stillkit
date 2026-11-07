//! Portable process identity, frozen before the first native window.
use std::sync::OnceLock;

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

static IDENTITY: OnceLock<Identity> = OnceLock::new();

pub(crate) fn current() -> &'static Identity {
    IDENTITY.get_or_init(Identity::default)
}

/// Sets the process identity before creating windows. Repeating the same identity succeeds;
/// changing a frozen identity returns the identity already in use.
/// Windows uses native class names; Wayland uses `prefix` as app_id. macOS ignores the names.
pub fn set_identity(identity: Identity) -> Result<(), Identity> {
    let current = IDENTITY.get_or_init(|| identity);
    if *current == identity { Ok(()) } else { Err(*current) }
}