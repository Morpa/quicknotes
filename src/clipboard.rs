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

/// What the system clipboard currently holds: the plain text and, when the
/// source app also put rich `text/html` there (a web page, Google Meet, a
/// document), that HTML converted to markdown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pasted {
    pub text: String,
    pub markdown: Option<String>,
}

impl Pasted {
    /// The markdown when there is one, the plain text otherwise.
    pub fn best(self) -> String {
        self.markdown.unwrap_or(self.text)
    }
}

pub fn paste() -> Result<Pasted> {
    let mut guard = CLIPBOARD
        .lock()
        .map_err(|_| anyhow::anyhow!("clipboard lock poisoned"))?;

    if guard.is_none() {
        *guard = Some(Clipboard::new().context("opening the system clipboard")?);
    }
    let clipboard = guard.as_mut().context("clipboard unavailable")?;

    let markdown = clipboard
        .get()
        .html()
        .ok()
        .and_then(|html| html_to_markdown(&html));
    let text = clipboard.get_text().unwrap_or_default();

    if text.is_empty() && markdown.is_none() {
        anyhow::bail!("the clipboard has no text");
    }
    Ok(Pasted { text, markdown })
}

/// Converts clipboard HTML to markdown. `None` when nothing readable remains.
pub fn html_to_markdown(html: &str) -> Option<String> {
    let converter = htmd::HtmlToMarkdown::builder()
        .options(htmd::options::Options {
            bullet_list_marker: htmd::options::BulletListMarker::Dash,
            ul_bullet_spacing: 1,
            ol_number_spacing: 1,
            ..Default::default()
        })
        .build();
    let markdown = converter.convert(html).ok()?;
    let markdown = markdown.replace("\r\n", "\n");
    let markdown = markdown.trim();
    (!markdown.is_empty()).then(|| markdown.to_owned())
}

/// Collapses whitespace so plain text and its markdown source can be compared.
pub fn same_text(a: &str, b: &str) -> bool {
    let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    words(a) == words(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_markdown() {
        let md =
            html_to_markdown("<p>Hello <strong>world</strong></p><ul><li>a</li><li>b</li></ul>")
                .unwrap();
        assert!(md.contains("Hello **world**"), "{md}");
        assert!(md.contains("a") && md.contains("- "), "{md}");
    }

    #[test]
    fn empty_html_is_none() {
        assert_eq!(html_to_markdown("<div> </div>"), None);
    }

    #[test]
    fn same_text_ignores_whitespace() {
        assert!(same_text("a  b\n\nc", " a b c "));
        assert!(!same_text("a b", "a c"));
    }
}
