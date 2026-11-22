//! Full-screen text snapshots rendered with `TestBackend`.
//!
//! Regenerate after an intentional UI change with
//! `UPDATE_SNAPSHOTS=1 cargo test --test snapshots`, then review the diff.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use quicknotes::app::App;
use quicknotes::config::{Config, Icons};
use quicknotes::db::{Database, NewItem};
use quicknotes::event::Action;
use quicknotes::ui::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn app(width: u16, height: u16) -> App {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(
        &NewItem::note("Async runtime")
            .body("# Tokio\n\nA **multi-threaded** runtime.\n\n- tasks\n- timers\n- io")
            .tags(["rust", "async"]),
    )
    .unwrap();
    db.insert_item(&NewItem::note("Groceries").body("milk, eggs").tags(["home"]))
        .unwrap();
    db.insert_item(&NewItem::todo("Write the README").priority(2).tags(["docs"]))
        .unwrap();
    db.insert_item(&NewItem::todo("Review PR").body("check the **tests**"))
        .unwrap();
    let done = db.insert_item(&NewItem::todo("Ship phase 4")).unwrap();
    db.set_done(done.id, true).unwrap();
    db.insert_item(
        &NewItem::snippet("Entry API", "rust")
            .body("let count = map.entry(key).or_insert(0);\n*count += 1;")
            .tags(["rust"]),
    )
    .unwrap();

    // ASCII icons keep the snapshots readable in any editor.
    let config = Config {
        icons: Icons::Ascii,
        ..Config::default()
    };
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, width, height));
    app
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
        .unwrap();
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        press(app, KeyCode::Char(character));
    }
}

/// Renders the app; the clock in the header is replaced so the output is stable.
fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| quicknotes::ui::draw(frame, app))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let mut lines: Vec<String> = (0..height)
        .map(|y| {
            let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
            row.trim_end().to_string()
        })
        .collect();
    lines[0] = " QUICKNOTES".to_string();
    lines.join("\n") + "\n"
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"));

    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {}; run with UPDATE_SNAPSHOTS=1",
            path.display()
        )
    });
    assert_eq!(
        expected, actual,
        "snapshot `{name}` changed; run with UPDATE_SNAPSHOTS=1 if intended"
    );
}

#[test]
fn notes_wide() {
    let app = app(100, 24);
    assert_snapshot("notes_wide", &screen(&app, 100, 24));
}

#[test]
fn todos_with_done_and_priority() {
    let mut app = app(100, 24);
    app.update(Action::NextSection).unwrap();
    assert_snapshot("todos", &screen(&app, 100, 24));
}

#[test]
fn snippet_preview() {
    let mut app = app(100, 24);
    app.update(Action::PrevSection).unwrap();
    assert_snapshot("snippet", &screen(&app, 100, 24));
}

#[test]
fn tag_filter_and_search() {
    let mut app = app(100, 24);
    press(&mut app, KeyCode::Char('#'));
    type_text(&mut app, "rust");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "async");
    assert_snapshot("filter_and_search", &screen(&app, 100, 24));
}

#[test]
fn editor_for_a_todo() {
    let mut app = app(100, 24);
    app.update(Action::NextSection).unwrap();
    app.update(Action::Edit).unwrap();
    assert_snapshot("editor_todo", &screen(&app, 100, 24));
}

#[test]
fn command_palette() {
    let mut app = app(100, 24);
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL))
        .unwrap();
    type_text(&mut app, "go");
    assert_snapshot("palette", &screen(&app, 100, 24));
}

#[test]
fn tag_picker() {
    let mut app = app(100, 24);
    press(&mut app, KeyCode::Char('t'));
    assert_snapshot("tag_picker", &screen(&app, 100, 24));
}

#[test]
fn help_overlay() {
    let mut app = app(100, 30);
    press(&mut app, KeyCode::Char('?'));
    assert_snapshot("help", &screen(&app, 100, 30));
}

#[test]
fn help_overlay_in_two_columns() {
    let mut app = app(130, 24);
    press(&mut app, KeyCode::Char('?'));
    assert_snapshot("help_wide", &screen(&app, 130, 24));
}

#[test]
fn narrow_terminal_shows_only_the_list() {
    let app = app(44, 16);
    assert_snapshot("narrow", &screen(&app, 44, 16));
}

#[test]
fn delete_confirmation() {
    let mut app = app(100, 24);
    press(&mut app, KeyCode::Delete);
    assert_snapshot("confirm_delete", &screen(&app, 100, 24));
}
