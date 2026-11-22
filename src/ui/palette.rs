//! Command palette and help overlay.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::db::Kind;
use crate::fuzzy::fuzzy_match;
use crate::ui::theme::Theme;
use crate::ui::widgets::{display_width, truncate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    New,
    Edit,
    Delete,
    ToggleDone,
    CyclePriority,
    CycleSort,
    Copy,
    TagItem,
    FilterTag,
    Search,
    ClearFilters,
    Go(Kind),
    Refresh,
    Help,
    Quit,
}

impl Command {
    pub const ALL: [Command; 17] = [
        Self::New,
        Self::Edit,
        Self::Delete,
        Self::ToggleDone,
        Self::CyclePriority,
        Self::CycleSort,
        Self::Copy,
        Self::TagItem,
        Self::FilterTag,
        Self::Search,
        Self::ClearFilters,
        Self::Go(Kind::Note),
        Self::Go(Kind::Todo),
        Self::Go(Kind::Snippet),
        Self::Refresh,
        Self::Help,
        Self::Quit,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::New => "New item",
            Self::Edit => "Edit item",
            Self::Delete => "Delete item",
            Self::ToggleDone => "Complete / reopen todo",
            Self::CyclePriority => "Cycle todo priority",
            Self::CycleSort => "Cycle sort order",
            Self::Copy => "Copy body to clipboard",
            Self::TagItem => "Tag item…",
            Self::FilterTag => "Filter by tag…",
            Self::Search => "Search",
            Self::ClearFilters => "Clear search and tag filter",
            Self::Go(Kind::Note) => "Go to notes",
            Self::Go(Kind::Todo) => "Go to todos",
            Self::Go(Kind::Snippet) => "Go to snippets",
            Self::Refresh => "Reload from database",
            Self::Help => "Show keyboard help",
            Self::Quit => "Quit",
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::New => "n",
            Self::Edit => "e",
            Self::Delete => "del",
            Self::ToggleDone => "space",
            Self::CyclePriority => "p",
            Self::CycleSort => "s",
            Self::Copy => "y",
            Self::TagItem => "t",
            Self::FilterTag => "#",
            Self::Search => "/",
            Self::ClearFilters => "esc",
            Self::Go(_) => "tab",
            Self::Refresh => "r",
            Self::Help => "?",
            Self::Quit => "q",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Palette {
    pub query: String,
    /// Commands matching the query, best first, with matched char indices.
    pub rows: Vec<(Command, Vec<usize>)>,
    pub cursor: usize,
}

impl Palette {
    pub fn new() -> Self {
        let mut palette = Self {
            query: String::new(),
            rows: Vec::new(),
            cursor: 0,
        };
        palette.rebuild();
        palette
    }

    pub fn current(&self) -> Option<Command> {
        self.rows.get(self.cursor).map(|(command, _)| *command)
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

    fn rebuild(&mut self) {
        let mut scored: Vec<(i32, usize, Command, Vec<usize>)> = Command::ALL
            .iter()
            .enumerate()
            .filter_map(|(order, command)| {
                fuzzy_match(&self.query, command.label())
                    .map(|found| (found.score, order, *command, found.indices))
            })
            .collect();

        // Best score first; ties keep the declaration order.
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.rows = scored
            .into_iter()
            .map(|(_, _, command, indices)| (command, indices))
            .collect();
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

fn popup(area: Rect, width: u16, height: u16) -> Option<Rect> {
    let width = area.width.saturating_sub(4).min(width);
    let height = area.height.saturating_sub(2).min(height);
    if width < 20 || height < 5 {
        return None;
    }
    Some(Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    })
}

fn framed(frame: &mut Frame, rect: Rect, title: &str, theme: &Theme) -> Rect {
    frame.render_widget(Clear, rect);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border())
        .style(Style::default().bg(theme.surface))
        .title(Line::from(Span::styled(
            format!(" {title} "),
            theme.accent(),
        )));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    }
}

pub fn draw_palette(frame: &mut Frame, area: Rect, palette: &Palette, theme: &Theme) {
    let wanted = palette.rows.len().max(1) as u16 + 5;
    let Some(rect) = popup(area, 54, wanted.max(9)) else {
        return;
    };
    let inner = framed(frame, rect, "COMMANDS", theme);
    if inner.width == 0 || inner.height < 4 {
        return;
    }

    let query = if palette.query.is_empty() {
        Span::styled("type a command…", theme.faint())
    } else {
        Span::styled(palette.query.clone(), theme.text())
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
    let offset = palette
        .cursor
        .saturating_sub(visible.saturating_sub(1))
        .min(palette.rows.len().saturating_sub(visible));

    let mut lines = Vec::new();
    if palette.rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no matching command",
            theme.faint(),
        )));
    }
    for (index, (command, matched)) in palette.rows.iter().enumerate().skip(offset).take(visible) {
        lines.push(command_line(
            *command,
            matched,
            index == palette.cursor,
            list.width as usize,
            theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines), list);

    let hints = [("enter", "run"), ("esc", "close")];
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

fn command_line(
    command: Command,
    matched: &[usize],
    selected: bool,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let base = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    let key = command.key();
    let label_width = width.saturating_sub(display_width(key) + 3);
    let label = truncate(command.label(), label_width);
    let gap = width.saturating_sub(display_width(&label) + display_width(key) + 2);

    let mut spans = vec![Span::styled(" ", base)];
    for (index, character) in label.chars().enumerate() {
        let style = if matched.contains(&index) {
            base.patch(theme.accent()).add_modifier(Modifier::BOLD)
        } else {
            base
        };
        spans.push(Span::styled(character.to_string(), style));
    }
    spans.push(Span::styled(" ".repeat(gap), base));
    spans.push(Span::styled(key, base.patch(theme.dimmed())));
    spans.push(Span::styled(" ", base));
    Line::from(spans)
}

const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "NAVIGATE",
        &[
            ("↑ ↓", "move / scroll preview"),
            ("← →", "switch pane"),
            ("tab / shift+tab", "next / previous section"),
            ("home end pgup pgdn", "jump"),
        ],
    ),
    (
        "ITEMS",
        &[
            ("n", "new item"),
            ("e / enter", "edit"),
            ("del", "delete"),
            ("space / x", "complete todo"),
            ("p", "todo priority"),
            ("y", "copy body"),
        ],
    ),
    (
        "FIND",
        &[
            ("/", "search (enter keeps it, esc clears)"),
            ("t / #", "tag item / filter by tag"),
            ("s", "sort order"),
            ("ctrl+k / :", "command palette"),
            ("esc", "clear search, then tag filter, then quit"),
        ],
    ),
    (
        "OTHER",
        &[("r", "reload"), ("?", "this help"), ("q / ctrl+c", "quit")],
    ),
];

pub fn draw_help(frame: &mut Frame, area: Rect, theme: &Theme) {
    let rows: usize = HELP.iter().map(|(_, items)| items.len() + 2).sum();
    let Some(rect) = popup(area, 62, rows as u16 + 3) else {
        return;
    };
    let inner = framed(frame, rect, "HELP", theme);
    if inner.width == 0 || inner.height < 2 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (section, items) in HELP {
        lines.push(Line::from(Span::styled(*section, theme.label())));
        for (key, label) in *items {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<20}"), theme.accent()),
                Span::styled(*label, theme.dimmed()),
            ]));
        }
        lines.push(Line::default());
    }
    lines.push(Line::from(Span::styled(
        "press any key to close",
        theme.faint(),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_lists_every_command_in_order() {
        let palette = Palette::new();
        assert_eq!(palette.rows.len(), Command::ALL.len());
        assert_eq!(palette.current(), Some(Command::New));
    }

    #[test]
    fn fuzzy_query_ranks_the_best_command_first() {
        let mut palette = Palette::new();
        for character in "sort".chars() {
            palette.push(character);
        }
        assert_eq!(palette.current(), Some(Command::CycleSort));

        palette.clear_query();
        for character in "todos".chars() {
            palette.push(character);
        }
        assert_eq!(palette.current(), Some(Command::Go(Kind::Todo)));
    }

    #[test]
    fn unmatched_query_has_no_rows() {
        let mut palette = Palette::new();
        for character in "zzzz".chars() {
            palette.push(character);
        }
        assert!(palette.rows.is_empty());
        assert_eq!(palette.current(), None);
    }
}
