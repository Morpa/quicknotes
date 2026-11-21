use std::time::Instant;

use quicknotes::app::{App, Pane, STATUS_TTL, Tone};
use quicknotes::config::Config;
use quicknotes::db::{Database, ItemQuery, Kind, NewItem};
use quicknotes::event::Action;
use quicknotes::ui::theme::Theme;
use ratatui::layout::Rect;

fn app_with_notes() -> App {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("First note")).unwrap();
    db.insert_item(&NewItem::note("Second note")).unwrap();

    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));
    app
}

#[test]
fn starts_on_notes_with_the_list_focused() {
    let app = app_with_notes();

    assert_eq!(app.section, Kind::Note);
    assert_eq!(app.focus, Pane::List);
    assert_eq!(app.items.len(), 2);
    assert_eq!(app.selected, 0);
    assert!(app.selected_item().is_some());
}

#[test]
fn selection_clamps_at_the_edges() {
    let mut app = app_with_notes();

    app.update(Action::Down).unwrap();
    assert_eq!(app.selected, 1);

    app.update(Action::Down).unwrap();
    assert_eq!(app.selected, 1);

    app.update(Action::Up).unwrap();
    assert_eq!(app.selected, 0);

    app.update(Action::Up).unwrap();
    assert_eq!(app.selected, 0);

    app.update(Action::Last).unwrap();
    assert_eq!(app.selected, 1);

    app.update(Action::First).unwrap();
    assert_eq!(app.selected, 0);
}

#[test]
fn sections_cycle_and_reload_the_list() {
    let mut app = app_with_notes();

    app.update(Action::NextSection).unwrap();
    assert_eq!(app.section, Kind::Todo);
    assert!(app.items.is_empty());
    assert_eq!(app.selected, 0);

    app.update(Action::NextSection).unwrap();
    assert_eq!(app.section, Kind::Snippet);

    app.update(Action::NextSection).unwrap();
    assert_eq!(app.section, Kind::Note);
    assert_eq!(app.items.len(), 2);
}

#[test]
fn focus_cycles_through_visible_panes() {
    let mut app = app_with_notes();
    assert_eq!(app.focus, Pane::List);

    app.update(Action::Right).unwrap();
    assert_eq!(app.focus, Pane::Preview);

    app.update(Action::Right).unwrap();
    assert_eq!(app.focus, Pane::Sidebar);

    app.update(Action::Left).unwrap();
    assert_eq!(app.focus, Pane::Preview);
}

#[test]
fn hidden_panes_lose_focus() {
    let mut app = app_with_notes();
    app.update(Action::Right).unwrap();
    assert_eq!(app.focus, Pane::Preview);

    app.set_area(Rect::new(0, 0, 40, 20));

    assert!(!app.visibility.sidebar);
    assert!(!app.visibility.preview);
    assert_eq!(app.focus, Pane::List);

    app.update(Action::Right).unwrap();
    assert_eq!(app.focus, Pane::List);
}

#[test]
fn refresh_keeps_the_selected_item() {
    let mut app = app_with_notes();
    app.update(Action::Down).unwrap();
    let selected_id = app.selected_item().unwrap().id;

    app.db.insert_item(&NewItem::note("Third note")).unwrap();
    app.update(Action::Refresh).unwrap();

    assert_eq!(app.items.len(), 3);
    assert_eq!(app.selected_item().unwrap().id, selected_id);
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Success)
    );
}

#[test]
fn status_expires_after_its_ttl() {
    let mut app = app_with_notes();
    app.set_status("saved", Tone::Success);
    app.tick();
    assert!(app.status.is_some());

    app.status.as_mut().unwrap().at = Instant::now() - STATUS_TTL;
    app.tick();
    assert!(app.status.is_none());
}

#[test]
fn preview_scrolls_long_notes() {
    let mut db = Database::open_in_memory().unwrap();
    let body = (1..=200)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    db.insert_item(&NewItem::note("Long note").body(body))
        .unwrap();

    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));

    assert!(app.preview_max_scroll > 0);

    app.update(Action::Right).unwrap();
    assert_eq!(app.focus, Pane::Preview);

    app.update(Action::Last).unwrap();
    assert_eq!(app.preview_scroll, app.preview_max_scroll);

    app.update(Action::First).unwrap();
    assert_eq!(app.preview_scroll, 0);
}

#[test]
fn sidebar_arrows_change_sections() {
    let mut app = app_with_notes();
    app.set_area(Rect::new(0, 0, 120, 32));
    app.update(Action::Right).unwrap();
    app.update(Action::Right).unwrap();
    assert_eq!(app.focus, Pane::Sidebar);

    app.update(Action::Down).unwrap();
    assert_eq!(app.section, Kind::Todo);

    app.update(Action::Up).unwrap();
    assert_eq!(app.section, Kind::Note);
}

#[test]
fn list_summary_reflects_todos() {
    let mut db = Database::open_in_memory().unwrap();
    let done = db.insert_item(&NewItem::todo("done")).unwrap();
    db.set_done(done.id, true).unwrap();
    db.insert_item(&NewItem::todo("open")).unwrap();

    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));
    app.update(Action::NextSection).unwrap();

    assert_eq!(app.section, Kind::Todo);
    assert_eq!(app.list_summary(), "1 open · 1 done");
    assert_eq!(app.items.len(), 2);
    assert_eq!(app.section_count(Kind::Todo), 1);

    let todos = app
        .db
        .list_items(&ItemQuery {
            kind: Some(Kind::Todo),
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(todos.len(), 2);
}
