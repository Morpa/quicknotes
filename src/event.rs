//! Keyboard input translated into application actions.

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self as crossterm_event, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
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
}

/// Waits up to `timeout` for a key press. Returns `None` on tick or on events
/// that do not map to an action (resize, focus, mouse, key release).
pub fn next_action(timeout: Duration) -> Result<Option<Action>> {
    if !crossterm_event::poll(timeout)? {
        return Ok(None);
    }

    let event = crossterm_event::read()?;
    Ok(match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => map_key(key),
        _ => None,
    })
}

fn map_key(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
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
