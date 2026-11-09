//! The clipboard display text copies and fields cut, copy and paste through ([`super::Runtime::set_clipboard`]).
//! A window host gives the runtime the system's (Wayland's `wl_data_device` on Linux; on
//! Windows display text uses the system clipboard and the focused field's RichEdit uses it directly); headless runtimes
//! and tests keep a [`Memory`] one, so a script never touches the user's clipboard.

/// Plain text in and out of a clipboard.
pub trait Clipboard {
    /// The text on the clipboard, if it holds text.
    fn get(&mut self) -> Option<String>;
    /// Set.
    fn set(&mut self, text: &str);
}

/// A clipboard in this process only: the default, and what tests use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Memory(pub Option<String>);

impl Clipboard for Memory {
    fn get(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn set(&mut self, text: &str) {
        self.0 = Some(text.into());
    }
}
