//! Small text helpers shared by the panes.

use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::ui::theme::Theme;

pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Clips text to `width` terminal cells, adding an ellipsis when needed.
/// Control characters are flattened so a stray newline cannot break the layout.
pub fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let sanitized: String = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();

    if UnicodeWidthStr::width(sanitized.as_str()) <= width {
        return sanitized;
    }
    if width == 1 {
        return "…".to_string();
    }

    let mut output = String::new();
    let mut used = 0;
    for character in sanitized.chars() {
        let character_width = character.width().unwrap_or(0);
        if used + character_width > width - 1 {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output.push('…');
    output
}

/// Tags as ` #tag ` chips on the surface color, separated by a space.
pub fn chips(tags: &[String], theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(tags.len() * 2);
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!(" #{tag} "),
            theme.tag().bg(theme.surface),
        ));
    }
    spans
}

pub fn relative_time(moment: DateTime<Utc>, now: DateTime<Local>) -> String {
    let seconds = now
        .with_timezone(&Utc)
        .signed_duration_since(moment)
        .num_seconds();

    if seconds < 60 {
        return "now".to_string();
    }
    if seconds < 3_600 {
        return format!("{}m", seconds / 60);
    }
    if seconds < 86_400 {
        return format!("{}h", seconds / 3_600);
    }

    let days = seconds / 86_400;
    if days == 1 {
        return "yesterday".to_string();
    }
    if days < 7 {
        return format!("{days}d");
    }

    let local = moment.with_timezone(&Local);
    if local.year() == now.year() {
        local.format("%d %b").to_string()
    } else {
        local.format("%d %b %Y").to_string()
    }
}

pub fn due_label(due: NaiveDate, today: NaiveDate) -> String {
    let delta = (due - today).num_days();
    if delta < 0 {
        return format!("{}d late", -delta);
    }

    match delta {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        2..=7 => format!("in {delta}d"),
        _ => {
            if due.year() == today.year() {
                due.format("%d %b").to_string()
            } else {
                due.format("%d %b %Y").to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn truncates_to_width() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hello", 1), "…");
        assert_eq!(truncate("hello", 0), "");
    }

    #[test]
    fn handles_wide_characters() {
        assert_eq!(truncate("日本語", 6), "日本語");
        assert_eq!(truncate("日本語", 4), "日…");
    }

    #[test]
    fn flattens_control_characters() {
        assert_eq!(truncate("a\nb", 10), "a b");
    }

    #[test]
    fn relative_times() {
        let now = Local::now();
        let utc = now.with_timezone(&Utc);

        assert_eq!(relative_time(utc - Duration::seconds(5), now), "now");
        assert_eq!(relative_time(utc - Duration::minutes(5), now), "5m");
        assert_eq!(relative_time(utc - Duration::hours(3), now), "3h");
        assert_eq!(relative_time(utc - Duration::hours(30), now), "yesterday");
        assert_eq!(relative_time(utc - Duration::days(3), now), "3d");
    }

    #[test]
    fn due_labels() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        assert_eq!(due_label(today, today), "today");
        assert_eq!(due_label(today + Duration::days(1), today), "tomorrow");
        assert_eq!(due_label(today + Duration::days(3), today), "in 3d");
        assert_eq!(due_label(today - Duration::days(2), today), "2d late");
    }
}
