//! Full-screen editor overlay built on `ratatui-textarea`.
//!
//! The editor owns the text buffers and the baseline snapshot used to decide
//! whether cancelling needs a confirmation. Rendering lives here too so the
//! overlay can keep its own field labels and footer.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui_textarea::{CursorMove, TextArea};

use crate::db::{Item, Kind};
use crate::markdown;
use crate::ui::theme::Theme;
use crate::ui::widgets::{display_width, truncate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Language,
    Body,
}

pub struct Editor {
    /// Database row being edited, when it already exists.
    pub item_id: Option<i64>,
    pub kind: Kind,
    /// Snapshot taken when the editor opened. `None` means the item is new.
    pub baseline: Option<Item>,
    pub field: Field,
    pub split: bool,
    pub dirty: bool,
    pub last_edit: Instant,
    pub error: Option<String>,
    title: TextArea<'static>,
    language: TextArea<'static>,
    body: TextArea<'static>,
}

impl Editor {
    pub fn new(kind: Kind, item: Option<&Item>, theme: &Theme) -> Self {
        let mut title = TextArea::new(vec![
            item.map(|item| item.title.clone()).unwrap_or_default(),
        ]);
        let mut language = TextArea::new(vec![
            item.and_then(|item| item.language.clone())
                .unwrap_or_default(),
        ]);
        let mut body = TextArea::new(vec![item.map(|item| item.body.clone()).unwrap_or_default()]);

        title.move_cursor(CursorMove::End);
        language.move_cursor(CursorMove::End);

        for area in [&mut title, &mut language, &mut body] {
            area.set_style(theme.text().bg(theme.surface));
            area.set_placeholder_style(theme.faint().bg(theme.surface));
            area.set_cursor_line_style(Style::default());
            area.set_cursor_style(Style::default().fg(theme.surface).bg(theme.accent));
            area.set_selection_style(theme.selected());
            area.set_tab_length(4);
        }

        title.set_placeholder_text("Untitled");
        language.set_placeholder_text("language");
        body.set_placeholder_text("Write something…");

        Self {
            item_id: item.map(|item| item.id),
            kind,
            baseline: item.cloned(),
            field: Field::Title,
            split: false,
            dirty: false,
            last_edit: Instant::now(),
            error: None,
            title,
            language,
            body,
        }
    }

    pub fn title_text(&self) -> String {
        self.title.lines().join("\n")
    }

    pub fn body_text(&self) -> String {
        self.body.lines().join("\n")
    }

    pub fn language_text(&self) -> String {
        self.language.lines().join("\n")
    }

    pub fn normalized_language(&self) -> Option<String> {
        let language = self.language_text().trim().to_lowercase();
        (!language.is_empty()).then_some(language)
    }

    /// Feeds a key to the focused text area. Returns whether the buffer changed.
    pub fn input(&mut self, key: KeyEvent) -> bool {
        let target = match self.field {
            Field::Title => &mut self.title,
            Field::Language => &mut self.language,
            Field::Body => &mut self.body,
        };

        // Enter switches fields outside the body; it never breaks a one-line
        // field into two. `Ctrl+M` is the textarea's alias for Enter.
        let newline = key.code == KeyCode::Enter
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('m'));
        if self.field != Field::Body && newline {
            return false;
        }

        target.input(key)
    }

    pub fn next_field(&mut self, forward: bool) {
        let order: &[Field] = if self.kind == Kind::Snippet {
            &[Field::Title, Field::Language, Field::Body]
        } else {
            &[Field::Title, Field::Body]
        };

        let current = order
            .iter()
            .position(|field| *field == self.field)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % order.len()
        } else {
            (current + order.len() - 1) % order.len()
        };
        self.field = order[next];
    }

    /// True when the buffers differ from the snapshot taken at open time.
    pub fn is_modified(&self) -> bool {
        match &self.baseline {
            Some(item) => {
                self.title_text().trim() != item.title
                    || self.body_text() != item.body
                    || self.normalized_language() != item.language
            }
            None => {
                !self.title_text().trim().is_empty()
                    || !self.body_text().trim().is_empty()
                    || !self.language_text().trim().is_empty()
            }
        }
    }
}

/// Draws the editor overlay. The caller is responsible for drawing the shell
/// underneath and any confirmation on top.
pub fn draw(frame: &mut Frame, area: Rect, editor: &Editor, theme: &Theme) {
    let popup = popup_rect(area);
    if popup.width == 0 || popup.height == 0 {
        return;
    }

    frame.render_widget(Clear, popup);

    let action = if editor.baseline.is_some() {
        "EDIT"
    } else {
        "NEW"
    };
    let label = format!(" {action} {} ", editor.kind.singular().to_uppercase());
    let mut title = vec![Span::styled(label, theme.accent())];
    if editor.dirty {
        title.push(Span::styled("●", theme.warning()));
        title.push(Span::raw(" "));
    }

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(Style::default().bg(theme.surface))
        .title(Line::from(title));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let footer_height = 1;
    let error_height = u16::from(editor.error.is_some());
    let content_height = inner.height.saturating_sub(footer_height + error_height);
    let content = Rect {
        x: inner.x.saturating_add(1),
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: content_height,
    };

    if content.width == 0 || content.height == 0 {
        return;
    }

    let (edit_area, preview_area) = if editor.split && content.width >= 70 {
        let columns = Layout::horizontal([
            Constraint::Percentage(55),
            Constraint::Length(1),
            Constraint::Min(24),
        ])
        .split(content);
        (columns[0], Some((columns[1], columns[2])))
    } else {
        (content, None)
    };

    render_fields(frame, edit_area, editor, theme);

    if let Some((separator, preview)) = preview_area {
        render_preview(frame, separator, preview, editor, theme);
    }

    let footer = Rect {
        y: inner.y + inner.height.saturating_sub(1),
        height: 1,
        ..inner
    };

    if let Some(error) = &editor.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {}", truncate(error, inner.width as usize)),
                theme.danger(),
            )),
            Rect {
                y: footer.y.saturating_sub(1),
                height: 1,
                ..inner
            },
        );
    }

    frame.render_widget(Paragraph::new(editor_hints(editor.field, theme)), footer);

    let (label, style) = if editor.dirty {
        ("● unsaved", theme.warning())
    } else if editor.item_id.is_some() {
        ("saved", theme.success())
    } else {
        ("new", theme.dimmed())
    };
    let width = display_width(label).min(footer.width as usize) as u16;
    if width > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(label, style)).alignment(Alignment::Right),
            Rect {
                x: footer.x + footer.width - width,
                width,
                ..footer
            },
        );
    }
}

fn render_fields(frame: &mut Frame, area: Rect, editor: &Editor, theme: &Theme) {
    let area = Rect {
        x: area.x.saturating_add(1),
        width: area.width.saturating_sub(2),
        ..area
    };
    if area.width == 0 || area.height == 0 {
        return;
    }

    let mut y = area.y;
    render_label(frame, area, y, "TITLE", editor.field == Field::Title, theme);
    y += 1;
    frame.render_widget(&editor.title, text_input(area, y));
    y += 1;

    if editor.kind == Kind::Snippet {
        y += 1;
        render_label(
            frame,
            area,
            y,
            "LANGUAGE",
            editor.field == Field::Language,
            theme,
        );
        y += 1;
        frame.render_widget(&editor.language, text_input(area, y));
        y += 1;
    }

    y += 1;
    render_label(frame, area, y, "BODY", editor.field == Field::Body, theme);
    y += 1;

    let height = area.bottom().saturating_sub(y);
    if height == 0 {
        return;
    }
    let body = Rect {
        x: area.x,
        y,
        width: area.width,
        height,
    };
    frame.render_widget(&editor.body, body);
}

fn render_label(frame: &mut Frame, area: Rect, y: u16, text: &str, focused: bool, theme: &Theme) {
    if y >= area.bottom() {
        return;
    }

    let style = if focused {
        theme.accent()
    } else {
        theme.label()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(format!(" {text}"), style)),
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        },
    );
}

fn text_input(area: Rect, y: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y,
        width: area.width.saturating_sub(1),
        height: 1,
    }
}

fn render_preview(
    frame: &mut Frame,
    separator: Rect,
    preview: Rect,
    editor: &Editor,
    theme: &Theme,
) {
    if separator.height > 0 {
        frame.render_widget(
            Paragraph::new(vec!["│"; separator.height as usize].join("\n")).style(theme.border()),
            separator,
        );
    }

    let inner = Rect {
        x: preview.x.saturating_add(1),
        width: preview.width.saturating_sub(1),
        ..preview
    };
    if inner.width == 0 || inner.height < 2 {
        return;
    }

    frame.render_widget(
        Paragraph::new(Span::styled(" PREVIEW", theme.label())),
        Rect { height: 1, ..inner },
    );
    frame.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(theme.border()),
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        },
    );

    let body_area = Rect {
        y: inner.y + 2,
        height: inner.height.saturating_sub(2),
        ..inner
    };
    if body_area.height == 0 {
        return;
    }

    let body = editor.body_text();
    let text = markdown::render(&body, editor.kind, theme, body_area.width);
    let cursor_line = editor.body.cursor().0;
    let scroll = cursor_line.saturating_sub(2).min(u16::MAX as usize) as u16;

    let mut paragraph = Paragraph::new(text).style(theme.text()).scroll((scroll, 0));
    if editor.kind != Kind::Snippet {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    frame.render_widget(paragraph, body_area);
}

fn editor_hints(field: Field, theme: &Theme) -> Line<'static> {
    let pairs: &'static [(&'static str, &'static str)] = match field {
        Field::Body => &[
            ("ctrl+s", "save"),
            ("tab", "indent"),
            ("shift+tab", "field"),
            ("ctrl+p", "preview"),
            ("esc", "cancel"),
        ],
        _ => &[
            ("ctrl+s", "save"),
            ("tab", "next field"),
            ("ctrl+p", "preview"),
            ("esc", "cancel"),
        ],
    };

    hint_line(pairs, theme)
}

fn hint_line(pairs: &'static [(&'static str, &'static str)], theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (index, (key, label)) in pairs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", theme.faint()));
        }
        spans.push(Span::styled(*key, theme.accent()));
        spans.push(Span::styled(format!(" {label}"), theme.dimmed()));
    }
    Line::from(spans)
}

fn popup_rect(area: Rect) -> Rect {
    let width = if area.width > 44 {
        area.width.min(110) - 4
    } else {
        area.width
    };
    let height = if area.height > 14 {
        area.height.min(42) - 2
    } else {
        area.height
    };

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
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
    fn new_editor_is_not_modified_until_something_is_typed() {
        let editor = Editor::new(Kind::Note, None, &theme());
        assert!(!editor.is_modified());

        let mut editor = editor;
        editor.title.insert_str("Hello");
        assert!(editor.is_modified());
    }

    #[test]
    fn fields_skip_language_for_notes() {
        let mut editor = Editor::new(Kind::Note, None, &theme());
        assert_eq!(editor.field, Field::Title);
        editor.next_field(true);
        assert_eq!(editor.field, Field::Body);
        editor.next_field(true);
        assert_eq!(editor.field, Field::Title);
    }

    #[test]
    fn fields_include_language_for_snippets() {
        let mut editor = Editor::new(Kind::Snippet, None, &theme());
        editor.next_field(true);
        assert_eq!(editor.field, Field::Language);
        editor.next_field(true);
        assert_eq!(editor.field, Field::Body);
    }
}
