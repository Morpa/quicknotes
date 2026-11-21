//! Pure layout math, shared by the renderer and the app state.
//!
//! Keeping this dependency-free (besides `ratatui` rectangles) lets `App` know
//! how tall the list is and how wide the preview is without owning a terminal.

use ratatui::layout::{Constraint, Layout, Rect};

pub const SIDEBAR_WIDTH: u16 = 20;

/// Which side panes fit in the current width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    pub sidebar: bool,
    pub preview: bool,
}

impl Visibility {
    pub const fn for_width(width: u16) -> Self {
        Self {
            sidebar: width >= 56,
            preview: width >= 78,
        }
    }
}

/// Rectangles for every region of the screen.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameLayout {
    pub header: Rect,
    pub rule_top: Rect,
    pub body: Rect,
    pub rule_bottom: Rect,
    pub footer: Rect,
    pub sidebar: Option<Rect>,
    pub sidebar_sep: Option<Rect>,
    pub list: Rect,
    pub list_header: Rect,
    pub list_rows: Rect,
    pub preview: Option<Rect>,
    pub preview_sep: Option<Rect>,
    pub preview_header: Rect,
    pub preview_body: Rect,
}

pub fn compute(area: Rect, visibility: Visibility) -> FrameLayout {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let body = rows[2];
    let (sidebar, sidebar_sep, list, preview_sep, preview) =
        if visibility.sidebar && visibility.preview {
            let columns = Layout::horizontal([
                Constraint::Length(SIDEBAR_WIDTH),
                Constraint::Length(1),
                Constraint::Percentage(40),
                Constraint::Length(1),
                Constraint::Min(24),
            ])
            .split(body);
            (
                Some(columns[0]),
                Some(columns[1]),
                columns[2],
                Some(columns[3]),
                Some(columns[4]),
            )
        } else if visibility.sidebar {
            let columns = Layout::horizontal([
                Constraint::Length(SIDEBAR_WIDTH),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(body);
            (Some(columns[0]), Some(columns[1]), columns[2], None, None)
        } else if visibility.preview {
            let columns = Layout::horizontal([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Min(24),
            ])
            .split(body);
            (None, None, columns[0], Some(columns[1]), Some(columns[2]))
        } else {
            (None, None, body, None, None)
        };

    let list_header = Rect {
        height: list.height.min(1),
        ..list
    };
    let list_rows = Rect {
        y: list.y.saturating_add(2),
        height: list.height.saturating_sub(2),
        ..list
    };

    let (preview_header, preview_body) = match preview {
        Some(rect) => {
            let inner = Rect {
                x: rect.x.saturating_add(1),
                width: rect.width.saturating_sub(2),
                ..rect
            };
            let header_height = inner.height.min(3);
            (
                Rect {
                    height: header_height,
                    ..inner
                },
                Rect {
                    y: inner.y.saturating_add(header_height),
                    height: inner.height.saturating_sub(header_height),
                    ..inner
                },
            )
        }
        None => (Rect::default(), Rect::default()),
    };

    FrameLayout {
        header: rows[0],
        rule_top: rows[1],
        body,
        rule_bottom: rows[3],
        footer: rows[4],
        sidebar,
        sidebar_sep,
        list,
        list_header,
        list_rows,
        preview,
        preview_sep,
        preview_header,
        preview_body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_layout_has_all_panes() {
        let layout = compute(Rect::new(0, 0, 120, 40), Visibility::for_width(120));
        assert!(layout.sidebar.is_some());
        assert!(layout.preview.is_some());
        assert_eq!(layout.header.height, 1);
        assert_eq!(layout.footer.height, 1);
        assert_eq!(layout.list_rows.height, layout.body.height - 2);
    }

    #[test]
    fn medium_layout_drops_the_preview() {
        let visibility = Visibility::for_width(64);
        assert!(visibility.sidebar);
        assert!(!visibility.preview);

        let layout = compute(Rect::new(0, 0, 64, 24), visibility);
        assert!(layout.sidebar.is_some());
        assert!(layout.preview.is_none());
    }

    #[test]
    fn narrow_layout_is_list_only() {
        let visibility = Visibility::for_width(40);
        assert!(!visibility.sidebar);
        assert!(!visibility.preview);

        let layout = compute(Rect::new(0, 0, 40, 20), visibility);
        assert!(layout.sidebar.is_none());
        assert!(layout.preview.is_none());
        assert_eq!(layout.list, layout.body);
    }
}
