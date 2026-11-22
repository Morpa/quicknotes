//! Tag picker overlay: assign tags to the selected item or filter the list.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::db::normalize_tag;
use crate::ui::theme::Theme;
use crate::ui::widgets::{display_width, truncate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPurpose {
    /// Toggle tags on the item with this id.
    Assign { item_id: i64 },
    /// Pick the tag the lists are filtered by.
    Filter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Tag,
    /// Typed text that matches no existing tag.
    Create,
    /// Removes the active tag filter.
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRow {
    pub name: String,
    pub count: u64,
    pub checked: bool,
    pub kind: RowKind,
}

#[derive(Debug, Clone)]
pub struct TagPicker {
    pub purpose: TagPurpose,
    pub query: String,
    pub rows: Vec<TagRow>,
    pub cursor: usize,
    all: Vec<(String, u64)>,
    selected: Vec<String>,
}

impl TagPicker {
    /// `all` is every tag with its item count; `selected` is the item's tags
    /// (assign) or the active filter (filter).
    pub fn new(purpose: TagPurpose, all: Vec<(String, u64)>, selected: Vec<String>) -> Self {
        let mut picker = Self {
            purpose,
            query: String::new(),
            rows: Vec::new(),
            cursor: 0,
            all,
            selected,
        };
        picker.rebuild();
        picker
    }

    /// Replaces the data after a change, keeping the query and the cursor.
    pub fn refresh(&mut self, all: Vec<(String, u64)>, selected: Vec<String>) {
        self.all = all;
        self.selected = selected;
        self.rebuild();
    }

    pub fn current(&self) -> Option<&TagRow> {
        self.rows.get(self.cursor)
    }

    pub fn push(&mut self, character: char) {
        self.query.push(character);
        self.cursor = 0;
        self.rebuild();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.cursor = 0;
        self.rebuild();
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.rebuild();
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() - 1;
        self.cursor = if delta < 0 {
            self.cursor.saturating_sub(delta.unsigned_abs())
        } else {
            (self.cursor + delta as usize).min(last)
        };
    }

    pub fn jump(&mut self, first: bool) {
        self.cursor = if first {
            0
        } else {
            self.rows.len().saturating_sub(1)
        };
    }

    fn rebuild(&mut self) {
        let needle = normalize_tag(&self.query);
        let mut rows = Vec::new();

        if self.purpose == TagPurpose::Filter && needle.is_empty() {
            rows.push(TagRow {
                name: "all tags".to_string(),
                count: 0,
                checked: self.selected.is_empty(),
                kind: RowKind::Clear,
            });
        }

        for (name, count) in &self.all {
            if needle.is_empty() || name.contains(&needle) {
                rows.push(TagRow {
                    name: name.clone(),
                    count: *count,
                    checked: self.selected.contains(name),
                    kind: RowKind::Tag,
                });
            }
        }

        let exists = self.all.iter().any(|(name, _)| *name == needle);
        if self.purpose != TagPurpose::Filter && !needle.is_empty() && !exists {
            rows.push(TagRow {
                name: needle,
                count: 0,
                checked: false,
                kind: RowKind::Create,
            });
        }

        self.rows = rows;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }
}

pub fn draw(frame: &mut Frame, area: Rect, picker: &TagPicker, theme: &Theme) {
    let width = area.width.saturating_sub(4).clamp(1, 44);
    let wanted = picker.rows.len().max(1) as u16 + 6;
    let height = wanted
        .min(area.height.saturating_sub(2))
        .min(18)
        .max(area.height.min(7));
    if width < 12 || height < 5 {
        return;
    }

    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, popup);

    let title = match picker.purpose {
        TagPurpose::Assign { .. } => " TAGS · assign ",
        TagPurpose::Filter => " TAGS · filter ",
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(Style::default().bg(theme.surface))
        .title(Line::from(Span::styled(title, theme.accent())));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    if inner.width == 0 || inner.height < 4 {
        return;
    }

    let query = if picker.query.is_empty() {
        Span::styled("type to search…", theme.faint())
    } else {
        Span::styled(picker.query.clone(), theme.text())
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("› ", theme.accent()), query])),
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

    let list = Rect {
        y: inner.y + 2,
        height: inner.height - 3,
        ..inner
    };
    let visible = list.height as usize;
    let offset = picker
        .cursor
        .saturating_sub(visible.saturating_sub(1))
        .min(picker.rows.len().saturating_sub(visible));

    let mut lines: Vec<Line<'static>> = Vec::new();
    if picker.rows.is_empty() {
        lines.push(Line::from(Span::styled("  no tags yet", theme.faint())));
    }
    for (index, row) in picker.rows.iter().enumerate().skip(offset).take(visible) {
        lines.push(row_line(
            row,
            index == picker.cursor,
            list.width as usize,
            picker,
            theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines), list);

    let hints = match picker.purpose {
        TagPurpose::Assign { .. } => [("space", "toggle"), ("esc", "done")],
        TagPurpose::Filter => [("enter", "select"), ("esc", "cancel")],
    };
    let mut spans = Vec::new();
    for (index, (key, label)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", theme.faint()));
        }
        spans.push(Span::styled(*key, theme.accent()));
        spans.push(Span::styled(format!(" {label}"), theme.dimmed()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            y: inner.y + inner.height - 1,
            height: 1,
            ..inner
        },
    );
}

fn row_line(
    row: &TagRow,
    selected: bool,
    width: usize,
    picker: &TagPicker,
    theme: &Theme,
) -> Line<'static> {
    let base = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    let mark = match (picker.purpose, row.kind) {
        (_, RowKind::Create) => "+",
        (TagPurpose::Assign { .. }, _) => {
            if row.checked {
                "◉"
            } else {
                "○"
            }
        }
        (TagPurpose::Filter, _) => {
            if row.checked {
                "●"
            } else {
                " "
            }
        }
    };

    let label = match row.kind {
        RowKind::Tag => format!("#{}", row.name),
        RowKind::Create => format!("create #{}", row.name),
        RowKind::Clear => row.name.clone(),
    };
    let count = if row.kind == RowKind::Tag {
        row.count.to_string()
    } else {
        String::new()
    };

    let name_width = width.saturating_sub(display_width(&count) + 4);
    let label = truncate(&label, name_width);
    let gap = width.saturating_sub(display_width(&label) + display_width(&count) + 3);

    let label_style = match row.kind {
        RowKind::Tag => base.patch(theme.tag()),
        RowKind::Create => base.patch(theme.success()),
        RowKind::Clear => base,
    };
    let label_style = if row.checked {
        label_style.add_modifier(Modifier::BOLD)
    } else {
        label_style
    };

    Line::from(vec![
        Span::styled(" ", base),
        Span::styled(mark, base.patch(theme.accent())),
        Span::styled(" ", base),
        Span::styled(label, label_style),
        Span::styled(" ".repeat(gap), base),
        Span::styled(count, base.patch(theme.dimmed())),
        Span::styled(" ", base),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags() -> Vec<(String, u64)> {
        vec![("cli".into(), 2), ("rust".into(), 3), ("tui".into(), 1)]
    }

    #[test]
    fn filter_lists_a_clear_row_first() {
        let picker = TagPicker::new(TagPurpose::Filter, tags(), vec!["rust".into()]);
        assert_eq!(picker.rows[0].kind, RowKind::Clear);
        assert!(!picker.rows[0].checked);
        assert!(
            picker
                .rows
                .iter()
                .any(|row| row.name == "rust" && row.checked)
        );
    }

    #[test]
    fn typing_narrows_and_offers_to_create_new_tags() {
        let mut picker = TagPicker::new(TagPurpose::Assign { item_id: 1 }, tags(), vec![]);
        for character in "ru".chars() {
            picker.push(character);
        }
        assert_eq!(picker.rows.len(), 2);
        assert_eq!(picker.rows[0].name, "rust");
        assert_eq!(picker.rows[1].kind, RowKind::Create);
        assert_eq!(picker.rows[1].name, "ru");

        picker.clear_query();
        for character in "#Rust".chars() {
            picker.push(character);
        }
        assert_eq!(picker.rows.len(), 1);
        assert_eq!(picker.rows[0].kind, RowKind::Tag);
    }

    #[test]
    fn cursor_stays_inside_the_rows() {
        let mut picker = TagPicker::new(TagPurpose::Assign { item_id: 1 }, tags(), vec![]);
        picker.move_cursor(10);
        assert_eq!(picker.cursor, 2);
        picker.move_cursor(-10);
        assert_eq!(picker.cursor, 0);
        picker.jump(false);
        assert_eq!(picker.cursor, 2);
    }
}
