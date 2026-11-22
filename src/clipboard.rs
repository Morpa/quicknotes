//! System clipboard access.

use std::sync::Mutex;

use anyhow::{Context, Result};
use arboard::Clipboard;

/// Kept for the life of the process: on X11 and Wayland the clipboard content
/// disappears when the owning `Clipboard` is dropped.
static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);

pub fn copy(text: &str) -> Result<()> {
    let mut guard = CLIPBOARD
        .lock()
        .map_err(|_| anyhow::anyhow!("clipboard lock poisoned"))?;

    if guard.is_none() {
        *guard = Some(Clipboard::new().context("opening the system clipboard")?);
    }

    guard
        .as_mut()
        .context("clipboard unavailable")?
        .set_text(text.to_owned())
        .context("writing to the system clipboard")
}
