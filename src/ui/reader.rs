//! Full-screen reader: the whole note or snippet, wrapped to the terminal
//! width and scrollable, so long text can be read and copied comfortably.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use crate::app::App;
use crate::db::Item;
use crate::markdown;
use crate::ui::theme::Theme;
use crate::ui::widgets::{display_width, truncate};

/// Rows above the body: title, meta line and a rule.
const HEADER_ROWS: u16 = 3;
/// Rows below the body: a rule and the hints.
const FOOTER_ROWS: u16 = 2;
const MARGIN_LEFT: u16 = 2;
/// Room for the scrollbar plus breathing space.
const MARGIN_RIGHT: u16 = 3;

/// Where the body text goes inside `area`.
pub fn body_rect(area: Rect) -> Rect {
    let top = HEADER_ROWS.min(area.height);
    let bottom = FOOTER_ROWS.min(area.height - top);
    Rect {
        x: area.x.saturating_add(MARGIN_LEFT),
        y: area.y + top,
        width: area.width.saturating_sub(MARGIN_LEFT + MARGIN_RIGHT),
        height: area.height - top - bottom,
    }
}

/// Number of wrapped lines `item` takes at `width` columns.
pub fn line_count(item: &Item, theme: &Theme, width: u16) -> usize {
    if width == 0 || item.body.trim().is_empty() {
        return 0;
    }
    let text = markdown::render(
        &item.body,
        item.kind,
        item.language.as_deref(),
        theme,
        width,
    );
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .line_count(width)
}

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some(item) = app.selected_item() else {
        return;
    };
    if area.width < 8 || area.height < 6 {
        return;
    }
    let theme = &app.theme;

    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(theme.text()), area);

    let inner_width = area.width.saturating_sub(MARGIN_LEFT * 2) as usize;
    let row = |y: u16| Rect {
        x: area.x + MARGIN_LEFT,
        y: area.y + y,
        width: inner_width as u16,
        height: 1,
    };

    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate(&item.title, inner_width),
            theme.title(),
        )),
        row(0),
    );
    frame.render_widget(Paragraph::new(meta_line(item, theme)), row(1));
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(theme.border()),
        Rect {
            y: area.y + 2,
            height: 1,
            ..area
        },
    );

    let body = body_rect(area);
    if body.height == 0 || body.width == 0 {
        return;
    }

    if item.body.trim().is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("(no content yet)", theme.faint())),
            body,
        );
    } else {
        let text = markdown::render(
            &item.body,
            item.kind,
            item.language.as_deref(),
            theme,
            body.width,
        );
        let scroll = app.reader_scroll.min(u16::MAX as usize) as u16;
        frame.render_widget(
            Paragraph::new(text)
                .style(theme.text())
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            body,
        );

        if app.reader_max_scroll > 0 {
            let mut state = ScrollbarState::new(app.reader_max_scroll + body.height as usize)
                .viewport_content_length(body.height as usize)
                .position(app.reader_scroll);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_symbol(Some("│"))
                    .track_style(theme.faint())
                    .thumb_symbol("█")
                    .thumb_style(theme.accent()),
                Rect {
                    x: area.x + area.width - 2,
                    ..body
                },
                &mut state,
            );
        }
    }

    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(theme.border()),
        Rect {
            y: area.y + area.height - 2,
            height: 1,
            ..area
        },
    );
    frame.render_widget(
        Paragraph::new(footer_line(
            app,
            item.kind == crate::db::Kind::Snippet,
            inner_width,
        )),
        row(area.height - 1),
    );
}

fn meta_line(item: &Item, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        item.kind.singular().to_uppercase(),
        theme.accent(),
    )];
    if let Some(language) = &item.language {
        spans.push(Span::styled(" · ", theme.faint()));
        spans.push(Span::styled(language.clone(), theme.tag()));
    }
    let lines = item.body.lines().count();
    spans.push(Span::styled(" · ", theme.faint()));
    spans.push(Span::styled(format!("{lines} lines"), theme.dimmed()));
    Line::from(spans)
}

fn footer_line(app: &App, snippet: bool, width: usize) -> Line<'static> {
    let theme = &app.theme;
    let copy = if snippet { "copy code" } else { "copy" };
    let pairs = [
        ("↑↓", "scroll"),
        ("pgup/pgdn", "page"),
        ("g/G", "top/end"),
        ("y", copy),
        ("e", "edit"),
        ("esc", "back"),
    ];

    let percent = if app.reader_max_scroll == 0 {
        "all".to_string()
    } else {
        format!("{}%", (app.reader_scroll * 100).div_ceil(app.reader_max_scroll))
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for (index, (key, label)) in pairs.iter().enumerate() {
        let gap = if index > 0 { 2 } else { 0 };
        let cost = gap + display_width(key) + 1 + display_width(label);
        if used + cost + percent.len() + 2 > width {
            break;
        }
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(*key, theme.accent()));
        spans.push(Span::styled(format!(" {label}"), theme.dimmed()));
        used += cost;
    }

    let status = app.status.as_ref().map(|status| status.text.clone());
    let right = status.unwrap_or(percent);
    let pad = width.saturating_sub(used + display_width(&right));
    spans.push(Span::styled(" ".repeat(pad), Style::default()));
    spans.push(Span::styled(right, theme.dimmed()));
    Line::from(spans)
}
