//! Keyboard input translated into application actions.

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self as crossterm_event, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    /// Esc: clears the search first, quits when there is none.
    Back,
    NextSection,
    PrevSection,
    Up,
    Down,
    Left,
    Right,
    First,
    Last,
    PageUp,
    PageDown,
    Refresh,
    New,
    Edit,
    /// Opens the full-screen reader.
    Read,
    Delete,
    ToggleDone,
    CyclePriority,
    CycleSort,
    Copy,
    Search,
    CycleTheme,
    TogglePin,
    Undo,
    ExternalEdit,
    Palette,
    Help,
}

/// Waits up to `timeout` for a key press. Returns `None` on tick or on events
/// that are not key presses (resize, focus, mouse, key release).
pub fn next_key(timeout: Duration) -> Result<Option<KeyEvent>> {
    Ok(match next_input(timeout)? {
        Some(Input::Key(key)) => Some(key),
        _ => None,
    })
}

/// A key press or a bracketed paste delivered as one block of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Key(KeyEvent),
    Paste(String),
}

/// Like [`next_key`], but keeps pastes whole instead of dropping them.
pub fn next_input(timeout: Duration) -> Result<Option<Input>> {
    if !crossterm_event::poll(timeout)? {
        return Ok(None);
    }

    Ok(match crossterm_event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => Some(Input::Key(key)),
        Event::Paste(text) => Some(Input::Paste(text)),
        _ => None,
    })
}

/// Maps a key press to an action in the normal (non-editor) mode.
pub fn map_key(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            KeyCode::Char('k') => Some(Action::Palette),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Char(' ') | KeyCode::Char('x') => Some(Action::ToggleDone),
        KeyCode::Char('p') => Some(Action::CyclePriority),
        KeyCode::Char('s') => Some(Action::CycleSort),
        KeyCode::Char('y') => Some(Action::Copy),
        KeyCode::Char('/') => Some(Action::Search),
        KeyCode::Char('T') => Some(Action::CycleTheme),
        KeyCode::Char('*') => Some(Action::TogglePin),
        KeyCode::Char('u') => Some(Action::Undo),
        KeyCode::Char('E') => Some(Action::ExternalEdit),
        KeyCode::Char(':') => Some(Action::Palette),
        KeyCode::Char('?') => Some(Action::Help),
        KeyCode::Char('n') => Some(Action::New),
        KeyCode::Char('e') | KeyCode::Enter => Some(Action::Edit),
        KeyCode::Char('v') => Some(Action::Read),
        KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('d') => Some(Action::Delete),
        KeyCode::Tab => Some(Action::NextSection),
        KeyCode::BackTab => Some(Action::PrevSection),
        KeyCode::Up => Some(Action::Up),
        KeyCode::Down => Some(Action::Down),
        KeyCode::Left => Some(Action::Left),
        KeyCode::Right => Some(Action::Right),
        KeyCode::Home => Some(Action::First),
        KeyCode::End => Some(Action::Last),
        KeyCode::PageUp => Some(Action::PageUp),
        KeyCode::PageDown => Some(Action::PageDown),
        KeyCode::Char('r') => Some(Action::Refresh),
        _ => None,
    }
}
