//! Rendering for the three-pane shell.

use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, Mode, Pane, Tone};
use crate::db::{Item, Kind, SortOrder};
use crate::markdown;
use crate::ui::layout::FrameLayout;
use crate::ui::theme::Theme;
use crate::ui::widgets::chips;
use crate::ui::widgets::{display_width, due_label, relative_time, truncate};

pub mod editor;
pub mod layout;
pub mod palette;
pub mod picker;
pub mod theme;
pub mod widgets;

pub fn draw(frame: &mut Frame, app: &App) {
    let frame_layout = layout::compute(frame.area(), app.visibility);

    render_header(frame, &frame_layout, app);
    render_rule(frame, frame_layout.rule_top, app);
    render_rule(frame, frame_layout.rule_bottom, app);

    if let Some(rect) = frame_layout.sidebar {
        render_sidebar(frame, rect, app);
    }
    if let Some(rect) = frame_layout.sidebar_sep {
        render_vline(frame, rect, app);
    }

    render_list(frame, &frame_layout, app);

    if let Some(rect) = frame_layout.preview_sep {
        render_vline(frame, rect, app);
    }
    if frame_layout.preview.is_some() {
        render_preview(frame, &frame_layout, app);
    }

    render_footer(frame, frame_layout.footer, app);

    if let Some(editor) = &app.editor
        && matches!(app.mode, Mode::Edit | Mode::ConfirmDiscard)
    {
        editor::draw(frame, frame.area(), editor, &app.theme);
    }

    if let Some(picker) = &app.tag_picker
        && app.mode == Mode::Tags
    {
        picker::draw(frame, frame.area(), picker, &app.theme);
    }

    if let Some(palette) = &app.palette
        && app.mode == Mode::Palette
    {
        palette::draw_palette(frame, frame.area(), palette, &app.theme);
    }
    if app.mode == Mode::Help {
        palette::draw_help(frame, frame.area(), &app.theme, app.help_scroll);
    }

    render_confirm(frame, app);
}

fn render_header(frame: &mut Frame, layout: &FrameLayout, app: &App) {
    let rect = layout.header;
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(" QUICKNOTES", app.theme.title()))),
        rect,
    );

    let stamp = format!("{} ", Local::now().format("%a %d %b · %H:%M"));
    let width = display_width(&stamp).min(rect.width as usize) as u16;
    if width == 0 {
        return;
    }

    let right = Rect {
        x: rect.x + rect.width - width,
        width,
        ..rect
    };
    frame.render_widget(
        Paragraph::new(Span::styled(stamp, app.theme.dimmed())).alignment(Alignment::Right),
        right,
    );
}

fn render_rule(frame: &mut Frame, rect: Rect, app: &App) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new("─".repeat(rect.width as usize)).style(app.theme.border()),
        rect,
    );
}

fn render_vline(frame: &mut Frame, rect: Rect, app: &App) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let text = vec!["│"; rect.height as usize].join("\n");
    frame.render_widget(Paragraph::new(text).style(app.theme.border()), rect);
}

fn render_sidebar(frame: &mut Frame, rect: Rect, app: &App) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let width = rect.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(Kind::ALL.len() + 10);
    for kind in Kind::ALL {
        lines.push(section_line(kind, width, app));
    }

    lines.push(Line::default());
    lines.push(Line::from(Span::styled(" TAGS", app.theme.label())));

    if app.tags.is_empty() {
        lines.push(Line::from(Span::styled("  no tags yet", app.theme.faint())));
    } else {
        for (index, (tag, count)) in app.sidebar_tags().iter().enumerate() {
            let row = Kind::ALL.len() + index;
            let cursor = app.focus == Pane::Sidebar && app.sidebar_row == row;
            lines.push(tag_line(tag, *count, width, cursor, app));
        }
    }

    frame.render_widget(Paragraph::new(lines), rect);
}

fn section_line(kind: Kind, width: usize, app: &App) -> Line<'static> {
    let selected = kind == app.section;
    let icon = app.icon(kind);
    let prefix = if selected { "▎ " } else { "  " };
    let icon_text = if icon.is_empty() {
        String::new()
    } else {
        format!("{icon} ")
    };
    let label = kind.label().to_uppercase();
    let count = app.section_count(kind).to_string();

    let left_width = display_width(prefix) + display_width(&icon_text) + display_width(&label);
    let gap = width.saturating_sub(left_width + display_width(&count) + 1);

    let bar_style = if selected {
        app.theme.accent()
    } else {
        app.theme.faint()
    };
    let icon_style = if selected {
        app.theme.accent()
    } else {
        app.theme.dimmed()
    };
    let label_style = if selected {
        app.theme.title()
    } else {
        app.theme.text()
    };
    let count_style = if selected {
        app.theme.text()
    } else {
        app.theme.dimmed()
    };

    Line::from(vec![
        Span::styled(prefix, bar_style),
        Span::styled(icon_text, icon_style),
        Span::styled(label, label_style),
        Span::styled(" ".repeat(gap), app.theme.text()),
        Span::styled(count, count_style),
        Span::raw(" "),
    ])
}

fn tag_line(tag: &str, count: u64, width: usize, cursor: bool, app: &App) -> Line<'static> {
    let active = app.tag_filter.as_deref() == Some(tag);
    let base = if cursor {
        app.theme.selected()
    } else {
        app.theme.text()
    };

    let marker = if active { "●" } else { " " };
    let label = format!(" #{tag}");
    let count = count.to_string();
    let label_width = width.saturating_sub(display_width(&count) + 3);
    let label = truncate(&label, label_width);
    let gap = width.saturating_sub(display_width(&label) + display_width(&count) + 3);

    let label_style = if active {
        base.patch(app.theme.accent()).add_modifier(Modifier::BOLD)
    } else {
        base.patch(app.theme.tag())
    };

    Line::from(vec![
        Span::styled(marker, base.patch(app.theme.accent())),
        Span::styled(label, label_style),
        Span::styled(" ".repeat(gap), base),
        Span::styled(count, base.patch(app.theme.dimmed())),
        Span::styled(" ", base),
    ])
}

fn render_list(frame: &mut Frame, layout: &FrameLayout, app: &App) {
    if layout.list.width == 0 || layout.list.height == 0 {
        return;
    }

    let mut header = vec![
        Span::styled(
            format!(" {}", app.section.label().to_uppercase()),
            app.theme.label(),
        ),
        Span::styled(" · ", app.theme.faint()),
        Span::styled(app.list_summary(), app.theme.dimmed()),
    ];
    if app.sort != SortOrder::Smart {
        header.push(Span::styled(" · ", app.theme.faint()));
        header.push(Span::styled(
            format!("by {}", app.sort.label()),
            app.theme.accent_alt(),
        ));
    }
    let header = Line::from(header);
    frame.render_widget(Paragraph::new(header), layout.list_header);

    if app.items.is_empty() {
        let filtered = !app.search.is_empty() || app.tag_filter.is_some();
        let (empty_title, empty_hint) = if filtered {
            ("  No matches", "  Press esc to clear the search or filter.")
        } else {
            (
                "  Nothing here yet",
                "  Items you create will show up in this list.",
            )
        };
        let lines = vec![
            Line::default(),
            Line::from(Span::styled(empty_title, app.theme.dimmed())),
            Line::from(Span::styled(empty_hint, app.theme.faint())),
        ];
        frame.render_widget(Paragraph::new(lines), layout.list_rows);
        return;
    }

    let start = app.list_offset.min(app.items.len());
    let visible = layout.list_rows.height as usize;
    let end = (start + visible).min(app.items.len());
    let width = layout.list.width as usize;
    let now = Local::now();

    let lines: Vec<Line<'static>> = app.items[start..end]
        .iter()
        .enumerate()
        .map(|(offset, item)| item_line(item, start + offset == app.selected, width, app, now))
        .collect();

    frame.render_widget(Paragraph::new(lines), layout.list_rows);
}

fn item_line(
    item: &Item,
    selected: bool,
    width: usize,
    app: &App,
    now: DateTime<Local>,
) -> Line<'static> {
    let theme = &app.theme;
    let base = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    let dim = base.patch(theme.dimmed());
    let accent = base.patch(theme.accent());
    let alt = base.patch(theme.accent_alt());
    let success = base.patch(theme.success());
    let warning = base.patch(theme.warning());
    let danger = base.patch(theme.danger());
    let done = base.patch(theme.done());

    let (meta, tone) = item_meta(item, now);
    let meta_width = display_width(&meta).min((width / 3).max(4));
    let meta = truncate(&meta, meta_width);

    let is_todo = item.kind == Kind::Todo;
    let (check, prefix_width) = if is_todo {
        (app.checkbox(item), 8)
    } else {
        ("", 4)
    };
    let title_width = width.saturating_sub(prefix_width + display_width(&meta) + 1);
    let title = truncate(&item.title, title_width);
    let gap = width.saturating_sub(prefix_width + display_width(&title) + display_width(&meta));

    let title_style = if item.done { done } else { base };
    let meta_style = match tone {
        Tone::Normal => dim,
        Tone::Success => success,
        Tone::Warning => warning,
        Tone::Danger => danger,
    };

    let mut spans = vec![
        Span::styled(if selected { "▎" } else { " " }, accent),
        Span::styled(" ", base),
        Span::styled(if item.pinned { "★" } else { " " }, alt),
        Span::styled(" ", base),
    ];
    if is_todo {
        spans.push(Span::styled(check, if item.done { success } else { dim }));
        spans.push(Span::styled(" ", base));
    }
    spans.push(Span::styled(title, title_style));
    spans.push(Span::styled(" ".repeat(gap), base));
    spans.push(Span::styled(meta, meta_style));

    Line::from(spans)
}

fn item_meta(item: &Item, now: DateTime<Local>) -> (String, Tone) {
    let today = now.date_naive();

    if item.kind != Kind::Todo {
        return (relative_time(item.updated_at, now), Tone::Normal);
    }

    let flag = if item.done || item.priority == 0 {
        String::new()
    } else {
        "!".repeat(usize::from(item.priority.min(2)))
    };

    let (label, mut tone) = match item.due {
        Some(due) => {
            let tone = if item.done {
                Tone::Normal
            } else if due < today {
                Tone::Danger
            } else if (due - today).num_days() <= 1 {
                Tone::Warning
            } else {
                Tone::Normal
            };
            (due_label(due, today), tone)
        }
        None => (relative_time(item.updated_at, now), Tone::Normal),
    };

    if item.priority >= 2 && tone == Tone::Normal && !item.done {
        tone = Tone::Warning;
    }

    if flag.is_empty() {
        (label, tone)
    } else {
        (format!("{flag} {label}"), tone)
    }
}

fn render_preview(frame: &mut Frame, layout: &FrameLayout, app: &App) {
    let Some(rect) = layout.preview else {
        return;
    };
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let Some(item) = app.selected_item() else {
        frame.render_widget(
            Paragraph::new(Span::styled("  Select an item", app.theme.faint())),
            rect,
        );
        return;
    };

    let width = layout.preview_header.width as usize;
    let header = layout.preview_header;
    let mut y = header.y;
    let bottom = header.y + header.height;

    if y < bottom {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(&item.title, width),
                app.theme.title(),
            )),
            Rect {
                y,
                height: 1,
                ..header
            },
        );
        y += 1;
    }

    if y < bottom {
        frame.render_widget(
            Paragraph::new(preview_meta_line(item, app)),
            Rect {
                y,
                height: 1,
                ..header
            },
        );
        y += 1;
    }

    if y < bottom {
        frame.render_widget(
            Paragraph::new("─".repeat(width)).style(app.theme.border()),
            Rect {
                y,
                height: 1,
                ..header
            },
        );
    }

    let body_rect = layout.preview_body;
    if body_rect.height == 0 {
        return;
    }

    let scroll = app.preview_scroll.min(u16::MAX as usize) as u16;

    if item.body.trim().is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("(no content yet)", app.theme.faint())),
            body_rect,
        );
        return;
    }

    let text = markdown::render(
        &item.body,
        item.kind,
        item.language.as_deref(),
        &app.theme,
        body_rect.width,
    );
    let mut paragraph = Paragraph::new(text)
        .style(app.theme.text())
        .scroll((scroll, 0));
    if item.kind != Kind::Snippet {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    frame.render_widget(paragraph, body_rect);
}

fn preview_meta_line(item: &Item, app: &App) -> Line<'static> {
    let theme = &app.theme;
    let mut spans = vec![Span::styled(
        item.kind.singular().to_uppercase(),
        theme.accent(),
    )];

    if let Some(language) = &item.language {
        spans.push(Span::styled(" · ", theme.faint()));
        spans.push(Span::styled(language.clone(), theme.tag()));
    }
    if item.kind == Kind::Todo && item.priority > 0 {
        let (label, style) = match item.priority {
            1 => ("medium priority", theme.warning()),
            _ => ("high priority", theme.danger()),
        };
        spans.push(Span::styled(" · ", theme.faint()));
        spans.push(Span::styled(label, style));
    }
    if !item.tags.is_empty() {
        spans.push(Span::styled(" · ", theme.faint()));
        spans.extend(chips(&item.tags, theme));
    }
    if let Some(due) = item.due {
        let today = Local::now().date_naive();
        let style = if !item.done && due < today {
            theme.danger()
        } else {
            theme.dimmed()
        };
        spans.push(Span::styled(" · ", theme.faint()));
        spans.push(Span::styled(due_label(due, today), style));
    }
    spans.push(Span::styled(" · ", theme.faint()));
    spans.push(Span::styled(
        relative_time(item.updated_at, Local::now()),
        theme.dimmed(),
    ));

    Line::from(spans)
}

fn render_footer(frame: &mut Frame, rect: Rect, app: &App) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let (right_text, right_style) = match &app.status {
        Some(status) => (status.text.clone(), tone_style(&app.theme, status.tone)),
        None => (selection_summary(app), app.theme.dimmed()),
    };

    let available = rect.width as usize;
    let right_width = display_width(&right_text)
        .min((available / 2).max(8))
        .min(available);
    let left_width = available.saturating_sub(right_width + 1);

    let left = if app.mode == Mode::Search {
        search_line(app)
    } else {
        hints_line(app)
    };
    frame.render_widget(
        Paragraph::new(left),
        Rect {
            width: left_width as u16,
            ..rect
        },
    );

    if right_width > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                truncate(&right_text, right_width),
                right_style,
            ))
            .alignment(Alignment::Right),
            Rect {
                x: rect.x + (available - right_width) as u16,
                width: right_width as u16,
                ..rect
            },
        );
    }
}

fn render_confirm(frame: &mut Frame, app: &App) {
    let (title, detail, style) = match app.mode {
        Mode::ConfirmDelete => {
            let Some(pending) = &app.pending_delete else {
                return;
            };
            (
                format!("Delete “{}”?", pending.title),
                "y confirm · n cancel",
                app.theme.danger(),
            )
        }
        Mode::ConfirmDiscard => (
            "Discard changes?".to_string(),
            "y discard · n keep editing",
            app.theme.warning(),
        ),
        Mode::Normal | Mode::Edit | Mode::Tags | Mode::Search | Mode::Palette | Mode::Help => {
            return;
        }
    };

    let area = frame.area();
    let width = area.width.saturating_sub(4).min(64).max(area.width.min(24));
    let height = area.height.min(5);
    if width == 0 || height == 0 {
        return;
    }

    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, popup);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(style)
        .style(Style::default().bg(app.theme.surface));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let lines = vec![
        Line::from(Span::styled(
            truncate(&title, inner.width as usize),
            style.add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        Line::from(Span::styled(detail, app.theme.dimmed())),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn search_line(app: &App) -> Line<'static> {
    Line::from(vec![
        Span::styled(" / ", app.theme.accent()),
        Span::styled(app.search.clone(), app.theme.text()),
        Span::styled("▏", app.theme.accent()),
        Span::styled("   enter keep · esc clear", app.theme.faint()),
    ])
}

fn hints_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let mut pairs: Vec<(&'static str, &'static str)> = match app.focus {
        Pane::Sidebar => vec![
            ("↑↓", "move"),
            ("enter", "filter tag"),
            ("n", "new"),
            ("q", "quit"),
        ],
        Pane::List => vec![("↑↓", "move"), ("n", "new"), ("e", "edit")],
        Pane::Preview => vec![("↑↓", "scroll"), ("←→", "pane"), ("e", "edit")],
    };

    if app.focus != Pane::Sidebar {
        match app.section {
            Kind::Todo => pairs.push(("space", "done")),
            Kind::Snippet => pairs.push(("y", "copy")),
            Kind::Note => {}
        }
        pairs.extend([
            ("/", "search"),
            ("t", "tags"),
            ("s", "sort"),
            ("?", "help"),
            ("q", "quit"),
        ]);
    }

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

fn selection_summary(app: &App) -> String {
    if app.items.is_empty() {
        "0 items".to_string()
    } else {
        format!("{}/{}", app.selected + 1, app.items.len())
    }
}

fn tone_style(theme: &Theme, tone: Tone) -> Style {
    match tone {
        Tone::Normal => theme.dimmed(),
        Tone::Success => theme.success(),
        Tone::Warning => theme.warning(),
        Tone::Danger => theme.danger(),
    }
}
