//! Full-screen editor overlay built on `ratatui-textarea`.
//!
//! The editor owns the text buffers and the baseline snapshot used to decide
//! whether cancelling needs a confirmation. Rendering lives here too so the
//! overlay can keep its own field labels and footer.

use std::time::Instant;

use chrono::{Duration, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui_textarea::{CursorMove, TextArea};

use crate::db::{Item, Kind, normalize_tags};
use crate::markdown;
use crate::ui::theme::Theme;
use crate::ui::widgets::{chips, display_width, truncate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Language,
    Due,
    Priority,
    Body,
    Tags,
}

/// Labels for the three priority levels stored in the database.
pub const PRIORITY_LABELS: [&str; 3] = ["none", "medium", "high"];

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
    pub priority: u8,
    title: TextArea<'static>,
    language: TextArea<'static>,
    due: TextArea<'static>,
    tags: TextArea<'static>,
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
        let mut due = TextArea::new(vec![
            item.and_then(|item| item.due)
                .map(format_due)
                .unwrap_or_default(),
        ]);
        let mut tags = TextArea::new(vec![
            item.map(|item| item.tags.join(", ")).unwrap_or_default(),
        ]);

        title.move_cursor(CursorMove::End);
        language.move_cursor(CursorMove::End);
        due.move_cursor(CursorMove::End);
        tags.move_cursor(CursorMove::End);

        for area in [&mut title, &mut language, &mut due, &mut tags, &mut body] {
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
        due.set_placeholder_text("none · today · tomorrow · +3d · 2026-12-31");
        tags.set_placeholder_text("comma-separated, e.g. rust, cli");

        Self {
            item_id: item.map(|item| item.id),
            kind,
            baseline: item.cloned(),
            field: Field::Title,
            split: false,
            dirty: false,
            last_edit: Instant::now(),
            error: None,
            priority: item.map_or(0, |item| item.priority.min(2)),
            title,
            language,
            due,
            tags,
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

    pub fn due_text(&self) -> String {
        self.due.lines().join("")
    }

    pub fn tags_text(&self) -> String {
        self.tags.lines().join(" ")
    }

    /// Tags typed in the tags field, normalized, sorted and deduplicated.
    pub fn tags_list(&self) -> Vec<String> {
        normalize_tags(
            self.tags_text()
                .split(|character: char| character == ',' || character.is_whitespace()),
        )
    }

    pub fn normalized_language(&self) -> Option<String> {
        let language = self.language_text().trim().to_lowercase();
        (!language.is_empty()).then_some(language)
    }

    /// Feeds a key to the focused text area. Returns whether the buffer changed.
    pub fn input(&mut self, key: KeyEvent) -> bool {
        if self.field == Field::Priority {
            return self.adjust_priority(key);
        }

        let target = match self.field {
            Field::Title => &mut self.title,
            Field::Language => &mut self.language,
            Field::Due => &mut self.due,
            Field::Tags => &mut self.tags,
            Field::Body => &mut self.body,
            Field::Priority => unreachable!("handled above"),
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

    /// Left/right (or `-`/`+`, space) change the priority; nothing is typed.
    fn adjust_priority(&mut self, key: KeyEvent) -> bool {
        let last = (PRIORITY_LABELS.len() - 1) as u8;
        let next = match key.code {
            KeyCode::Right | KeyCode::Char('+') | KeyCode::Char(' ') => {
                if self.priority >= last {
                    0
                } else {
                    self.priority + 1
                }
            }
            KeyCode::Left | KeyCode::Char('-') => {
                if self.priority == 0 {
                    last
                } else {
                    self.priority - 1
                }
            }
            _ => return false,
        };

        self.priority = next;
        true
    }

    /// Order in which Tab visits the fields. Tags come last so that Tab from
    /// the title still lands on the body.
    pub fn field_order(&self) -> &'static [Field] {
        match self.kind {
            Kind::Snippet => &[Field::Title, Field::Language, Field::Body, Field::Tags],
            Kind::Todo => &[
                Field::Title,
                Field::Due,
                Field::Priority,
                Field::Body,
                Field::Tags,
            ],
            Kind::Note => &[Field::Title, Field::Body, Field::Tags],
        }
    }

    pub fn focus(&mut self, field: Field) {
        if self.field_order().contains(&field) {
            self.field = field;
        }
    }

    pub fn next_field(&mut self, forward: bool) {
        let order = self.field_order();

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
                    || self.tags_list() != item.tags
                    || (self.kind == Kind::Todo
                        && (self.due_text().trim() != item.due.map(format_due).unwrap_or_default()
                            || self.priority != item.priority))
            }
            None => {
                !self.title_text().trim().is_empty()
                    || !self.body_text().trim().is_empty()
                    || !self.language_text().trim().is_empty()
                    || !self.tags_list().is_empty()
                    || (self.kind == Kind::Todo
                        && (!self.due_text().trim().is_empty() || self.priority != 0))
            }
        }
    }
}

pub fn format_due(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Parses the due field: empty clears the date; accepts `YYYY-MM-DD`,
/// `today`, `tomorrow`, `+Nd` and `+Nw`.
pub fn parse_due(input: &str, today: NaiveDate) -> Result<Option<NaiveDate>, String> {
    let input = input.trim().to_lowercase();
    let invalid = || format!("Invalid due date “{input}” (try 2026-12-31, tomorrow or +3d)");

    match input.as_str() {
        "" | "none" | "-" => return Ok(None),
        "today" => return Ok(Some(today)),
        "tomorrow" | "tmr" => return Ok(Some(today + Duration::days(1))),
        _ => {}
    }

    if let Some(offset) = input.strip_prefix('+') {
        let (digits, unit) = offset.split_at(offset.len().saturating_sub(1));
        let amount: i64 = digits.parse().map_err(|_| invalid())?;
        let days = match unit {
            "d" => amount,
            "w" => amount.checked_mul(7).ok_or_else(invalid)?,
            _ => return Err(invalid()),
        };
        return today
            .checked_add_signed(Duration::days(days))
            .map(Some)
            .ok_or_else(invalid);
    }

    NaiveDate::parse_from_str(&input, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| invalid())
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
    frame.render_widget(&editor.title, text_input(area, y + 1));
    y += 3;

    if editor.kind == Kind::Snippet {
        render_label(
            frame,
            area,
            y,
            "LANGUAGE",
            editor.field == Field::Language,
            theme,
        );
        frame.render_widget(&editor.language, text_input(area, y + 1));
        y += 3;
    }

    if editor.kind == Kind::Todo {
        render_todo_fields(frame, area, y, editor, theme);
        y += 3;
    }

    render_label(frame, area, y, "BODY", editor.field == Field::Body, theme);
    y += 1;

    // The tags block (label, input, chips) sits under the body.
    let tags_block = 3;
    let height = area.bottom().saturating_sub(y + tags_block + 1);
    if height > 0 {
        let body = Rect {
            x: area.x,
            y,
            width: area.width,
            height,
        };
        frame.render_widget(&editor.body, body);
    }
    y += height + 1;

    render_label(frame, area, y, "TAGS", editor.field == Field::Tags, theme);
    if y + 1 < area.bottom() {
        frame.render_widget(&editor.tags, text_input(area, y + 1));
    }
    if y + 2 < area.bottom() {
        let tags = editor.tags_list();
        let line = if tags.is_empty() {
            Line::from(Span::styled("  no tags", theme.faint()))
        } else {
            let mut spans = vec![Span::raw(" ")];
            spans.extend(chips(&tags, theme));
            Line::from(spans)
        };
        frame.render_widget(
            Paragraph::new(line),
            Rect {
                x: area.x,
                y: y + 2,
                width: area.width,
                height: 1,
            },
        );
    }
}

/// Due date on the left, priority selector on the right.
fn render_todo_fields(frame: &mut Frame, area: Rect, y: u16, editor: &Editor, theme: &Theme) {
    let half = area.width / 2;
    let left = Rect {
        width: half.saturating_sub(1).max(1),
        ..area
    };
    let right = Rect {
        x: area.x + half,
        width: area.width.saturating_sub(half),
        ..area
    };

    render_label(frame, left, y, "DUE", editor.field == Field::Due, theme);
    frame.render_widget(&editor.due, text_input(left, y + 1));

    let focused = editor.field == Field::Priority;
    render_label(frame, right, y, "PRIORITY", focused, theme);
    if y + 1 < right.bottom() {
        let value = PRIORITY_LABELS[usize::from(editor.priority.min(2))];
        let style = match editor.priority {
            2 => theme.danger(),
            1 => theme.warning(),
            _ => theme.dimmed(),
        };
        let arrows = if focused {
            theme.accent()
        } else {
            theme.faint()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("‹ ", arrows),
                Span::styled(value, style),
                Span::styled(" ›", arrows),
            ])),
            text_input(right, y + 1),
        );
    }
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
    let language = editor.normalized_language();
    let text = markdown::render(
        &body,
        editor.kind,
        language.as_deref(),
        theme,
        body_area.width,
    );
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
            ("ctrl+t", "tags"),
            ("ctrl+p", "preview"),
            ("esc", "cancel"),
        ],
        Field::Priority => &[
            ("ctrl+s", "save"),
            ("←→", "change"),
            ("tab", "next field"),
            ("esc", "cancel"),
        ],
        Field::Tags => &[
            ("ctrl+s", "save"),
            ("tab", "title"),
            ("ctrl+p", "preview"),
            ("esc", "cancel"),
        ],
        _ => &[
            ("ctrl+s", "save"),
            ("tab", "next field"),
            ("ctrl+t", "tags"),
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
        assert_eq!(editor.field, Field::Tags);
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

#[cfg(test)]
mod due_tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 22).unwrap()
    }

    #[test]
    fn parses_relative_and_absolute_dates() {
        let today = today();
        assert_eq!(parse_due("", today), Ok(None));
        assert_eq!(parse_due(" none ", today), Ok(None));
        assert_eq!(parse_due("Today", today), Ok(Some(today)));
        assert_eq!(
            parse_due("tomorrow", today),
            Ok(Some(today + Duration::days(1)))
        );
        assert_eq!(parse_due("+3d", today), Ok(Some(today + Duration::days(3))));
        assert_eq!(
            parse_due("+2w", today),
            Ok(Some(today + Duration::days(14)))
        );
        assert_eq!(
            parse_due("2026-12-31", today),
            Ok(NaiveDate::from_ymd_opt(2026, 12, 31))
        );
    }

    #[test]
    fn rejects_garbage() {
        for input in ["soon", "+d", "+3", "+3x", "2026-13-01", "31/12/2026", "+"] {
            assert!(parse_due(input, today()).is_err(), "{input}");
        }
    }
}
