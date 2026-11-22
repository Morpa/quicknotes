use std::cell::RefCell;
use std::rc::Rc;

use chrono::{Duration, Local};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use quicknotes::app::{App, Mode, Tone};
use quicknotes::config::Config;
use quicknotes::db::{Database, Kind, NewItem, SortOrder};
use quicknotes::event::Action;
use quicknotes::ui::theme::Theme;
use ratatui::layout::Rect;

fn app_with(db: Database) -> App {
    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));
    app
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
        .unwrap();
}

fn press_ctrl(app: &mut App, character: char) {
    app.handle_key(KeyEvent::new(
        KeyCode::Char(character),
        KeyModifiers::CONTROL,
    ))
    .unwrap();
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        press(app, KeyCode::Char(character));
    }
}

fn todo_app() -> App {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::todo("alpha")).unwrap();
    db.insert_item(&NewItem::todo("bravo")).unwrap();
    db.insert_item(&NewItem::todo("charlie")).unwrap();
    let mut app = app_with(db);
    app.update(Action::NextSection).unwrap();
    app
}

fn titles(app: &App) -> Vec<&str> {
    app.items.iter().map(|item| item.title.as_str()).collect()
}

#[test]
fn space_completes_a_todo_and_keeps_the_cursor_row() {
    let mut app = todo_app();
    assert_eq!(app.section, Kind::Todo);
    let first = app.selected_item().unwrap().title.clone();

    app.update(Action::ToggleDone).unwrap();

    assert_eq!(app.selected, 0);
    assert_ne!(app.selected_item().unwrap().title, first);
    assert_eq!(app.items.last().unwrap().title, first);
    assert!(app.items.last().unwrap().done);
    assert_eq!(app.stats.done_todos, 1);
    assert_eq!(app.stats.open_todos, 2);
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Success)
    );

    app.update(Action::Last).unwrap();
    app.update(Action::ToggleDone).unwrap();
    assert_eq!(app.stats.done_todos, 0);
    assert!(app.status.as_ref().unwrap().text.starts_with("Reopened"));
}

#[test]
fn completing_works_from_the_keyboard() {
    let mut app = todo_app();
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(app.stats.done_todos, 1);
    press(&mut app, KeyCode::Char('x'));
    assert_eq!(app.stats.done_todos, 2);
}

#[test]
fn only_todos_can_be_completed_or_prioritized() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("just a note")).unwrap();
    let mut app = app_with(db);

    app.update(Action::ToggleDone).unwrap();
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Warning)
    );

    app.update(Action::CyclePriority).unwrap();
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Warning)
    );
    assert_eq!(app.selected_item().unwrap().priority, 0);
}

#[test]
fn priority_cycles_and_follows_the_item() {
    let mut app = todo_app();
    let id = app.selected_item().unwrap().id;

    app.update(Action::CyclePriority).unwrap();
    assert_eq!(app.selected_item().unwrap().id, id);
    assert_eq!(app.selected_item().unwrap().priority, 1);

    app.update(Action::CyclePriority).unwrap();
    assert_eq!(app.selected_item().unwrap().priority, 2);

    app.update(Action::CyclePriority).unwrap();
    assert_eq!(app.selected_item().unwrap().priority, 0);
}

#[test]
fn sort_cycles_through_every_order() {
    let mut db = Database::open_in_memory().unwrap();
    let today = Local::now().date_naive();
    db.insert_item(&NewItem::todo("zeta").due(today + Duration::days(5)))
        .unwrap();
    db.insert_item(&NewItem::todo("alpha").priority(2)).unwrap();
    db.insert_item(&NewItem::todo("mid").due(today + Duration::days(1)))
        .unwrap();
    let mut app = app_with(db);
    app.update(Action::NextSection).unwrap();

    assert_eq!(app.sort, SortOrder::Smart);
    assert_eq!(titles(&app), ["mid", "zeta", "alpha"]);

    let mut seen = vec![app.sort];
    for _ in 0..5 {
        app.update(Action::CycleSort).unwrap();
        seen.push(app.sort);
        if app.sort == SortOrder::Title {
            assert_eq!(titles(&app), ["alpha", "mid", "zeta"]);
        }
        if app.sort == SortOrder::Priority {
            assert_eq!(titles(&app)[0], "alpha");
        }
        if app.sort == SortOrder::Due {
            assert_eq!(titles(&app), ["mid", "zeta", "alpha"]);
        }
    }
    assert_eq!(seen.len(), SortOrder::ALL.len());

    app.update(Action::CycleSort).unwrap();
    assert_eq!(app.sort, SortOrder::Smart);
}

#[test]
fn copy_sends_the_snippet_body_to_the_clipboard() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::snippet("entry", "rust").body("map.entry(key)"))
        .unwrap();
    let mut app = app_with(db);
    app.update(Action::PrevSection).unwrap();
    assert_eq!(app.section, Kind::Snippet);

    let copied = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = Rc::clone(&copied);
    app.clipboard = Box::new(move |text| {
        sink.borrow_mut().push(text.to_string());
        Ok(())
    });

    press(&mut app, KeyCode::Char('y'));

    assert_eq!(copied.borrow().as_slice(), ["map.entry(key)"]);
    let status = app.status.as_ref().unwrap();
    assert_eq!(status.tone, Tone::Success);
    assert!(status.text.contains("Copied"));
}

#[test]
fn copy_reports_clipboard_failures_and_empty_bodies() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::snippet("full", "sh").body("ls"))
        .unwrap();
    db.insert_item(&NewItem::snippet("empty", "sh")).unwrap();
    let mut app = app_with(db);
    app.update(Action::PrevSection).unwrap();

    app.clipboard = Box::new(|_| Err(anyhow::anyhow!("no display")));
    let full = app
        .items
        .iter()
        .position(|item| item.title == "full")
        .unwrap();
    let empty = app
        .items
        .iter()
        .position(|item| item.title == "empty")
        .unwrap();

    app.selected = full;
    app.update(Action::Copy).unwrap();
    assert_eq!(app.status.as_ref().unwrap().tone, Tone::Danger);

    app.selected = empty;
    app.update(Action::Copy).unwrap();
    assert_eq!(app.status.as_ref().unwrap().tone, Tone::Warning);
}

#[test]
fn editor_saves_due_and_priority_for_a_todo() {
    let mut app = todo_app();
    app.update(Action::New).unwrap();

    type_text(&mut app, "Ship it");
    press(&mut app, KeyCode::Tab); // due
    type_text(&mut app, "tomorrow");
    press(&mut app, KeyCode::Tab); // priority
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Right);
    press_ctrl(&mut app, 's');

    assert_eq!(app.mode, Mode::Normal);
    let item = app.selected_item().unwrap();
    assert_eq!(item.title, "Ship it");
    assert_eq!(
        item.due,
        Some(Local::now().date_naive() + Duration::days(1))
    );
    assert_eq!(item.priority, 2);
}

#[test]
fn invalid_due_blocks_an_explicit_save() {
    let mut app = todo_app();
    app.update(Action::New).unwrap();

    type_text(&mut app, "Broken");
    press(&mut app, KeyCode::Tab);
    type_text(&mut app, "next tuesday");
    press_ctrl(&mut app, 's');

    assert_eq!(app.mode, Mode::Edit);
    let editor = app.editor.as_ref().unwrap();
    assert!(editor.error.as_ref().unwrap().contains("Invalid due date"));
    assert_eq!(app.items.len(), 3);
}

#[test]
fn editing_a_todo_restores_due_and_priority() {
    let mut db = Database::open_in_memory().unwrap();
    let due = Local::now().date_naive() + Duration::days(9);
    db.insert_item(&NewItem::todo("plan").due(due).priority(1))
        .unwrap();
    let mut app = app_with(db);
    app.update(Action::NextSection).unwrap();
    app.update(Action::Edit).unwrap();

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.due_text(), due.format("%Y-%m-%d").to_string());
    assert_eq!(editor.priority, 1);
    assert!(!editor.is_modified());
}
