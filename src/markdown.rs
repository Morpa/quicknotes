//! Markdown rendering for the preview pane and the editor split view.
//!
//! Notes and todos are rendered as markdown; snippets stay verbatim until the
//! syntax-highlighting theme arrives in phase 4. Styles come from the active
//! Kanagawa palette, so no colors are defined here.

use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use tui_markdown::{AlertKind, Options, StyleSheet, from_str_with_options};

use crate::db::Kind;
use crate::ui::theme::Theme;

/// Markdown styles mapped onto the semantic roles of the active theme.
#[derive(Debug, Clone)]
pub struct KanagawaStyleSheet {
    theme: Theme,
}

impl KanagawaStyleSheet {
    pub fn new(theme: &Theme) -> Self {
        Self {
            theme: theme.clone(),
        }
    }
}

impl StyleSheet for KanagawaStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 | 2 => self.theme.accent().add_modifier(Modifier::BOLD),
            _ => self.theme.accent(),
        }
    }

    fn code(&self) -> Style {
        self.theme.success().bg(self.theme.surface)
    }

    fn link(&self) -> Style {
        self.theme.accent().add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        self.theme.tag().add_modifier(Modifier::ITALIC)
    }

    fn heading_meta(&self) -> Style {
        self.theme.dimmed()
    }

    fn metadata_block(&self) -> Style {
        self.theme.faint()
    }

    /// The preview is already styled by color, so heading markers would only
    /// add noise.
    fn heading_marker(&self, _level: u8) -> &str {
        ""
    }

    fn html(&self) -> Style {
        self.theme.faint()
    }

    fn math_inline(&self) -> Style {
        self.theme.violet().add_modifier(Modifier::ITALIC)
    }

    fn math_display(&self) -> Style {
        self.theme.violet()
    }

    fn footnote_ref(&self) -> Style {
        self.theme.dimmed().add_modifier(Modifier::ITALIC)
    }

    fn footnote_def(&self) -> Style {
        self.theme.dimmed()
    }

    fn definition_term(&self) -> Style {
        self.theme.text().add_modifier(Modifier::BOLD)
    }

    fn definition_description(&self) -> Style {
        self.theme.text()
    }

    fn alert(&self, kind: AlertKind) -> Style {
        match kind {
            AlertKind::Note => self.theme.accent(),
            AlertKind::Tip => self.theme.success(),
            AlertKind::Important => self.theme.violet(),
            AlertKind::Warning => self.theme.warning(),
            AlertKind::Caution => self.theme.danger(),
        }
    }

    /// Emoji icons are wider than one cell in several terminals; the labels
    /// keep alerts readable without them.
    fn alert_icon(&self, _kind: AlertKind) -> &str {
        ""
    }

    fn table_header(&self) -> Style {
        self.theme.accent().add_modifier(Modifier::BOLD)
    }

    fn table_cell(&self) -> Style {
        self.theme.text()
    }

    fn table_border(&self) -> Style {
        self.theme.border()
    }

    fn list_marker(&self) -> Style {
        self.theme.accent_alt()
    }

    fn image_alt(&self) -> Style {
        self.theme.faint().add_modifier(Modifier::ITALIC)
    }
}

/// Renders `body` for `kind` at `width` columns.
///
/// Snippets are returned verbatim: their language-aware highlighting lands in
/// phase 4.
pub fn render<'a>(body: &'a str, kind: Kind, theme: &Theme, width: u16) -> Text<'a> {
    if kind == Kind::Snippet {
        return Text::raw(body);
    }

    let options = Options::new(KanagawaStyleSheet::new(theme)).table_width(width.max(1));
    from_str_with_options(body, &options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Accent, Background, ColorMode, ThemeName};

    fn theme() -> Theme {
        Theme::new(
            ThemeName::Wave,
            Background::Terminal,
            Accent::CrystalBlue,
            ColorMode::Truecolor,
        )
    }

    #[test]
    fn renders_markdown_without_heading_markers() {
        let text = render(
            "# Title\n\nbody with **emphasis**",
            Kind::Note,
            &theme(),
            40,
        );
        let rendered = text.to_string();

        assert!(rendered.contains("Title"));
        assert!(rendered.contains("body with emphasis"));
        assert!(!rendered.contains("# Title"));
    }

    #[test]
    fn snippets_are_verbatim() {
        let text = render("# not a heading", Kind::Snippet, &theme(), 40);
        assert_eq!(text.to_string(), "# not a heading");
    }

    #[test]
    fn lists_and_tables_render() {
        let text = render(
            "- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |",
            Kind::Note,
            &theme(),
            40,
        );
        let rendered = text.to_string();

        assert!(rendered.contains("one"));
        assert!(rendered.contains("two"));
        assert!(rendered.contains('a'));
        assert!(rendered.contains('1'));
    }
}
