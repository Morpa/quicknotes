use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use quicknotes::app::{App, Tone, UNDO_LIMIT};
use quicknotes::config::{Config, ThemeName};
use quicknotes::db::{Database, Kind, NewItem};
use quicknotes::event::Action;
use quicknotes::ui::palette::Command;
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

fn tone(app: &App) -> Tone {
    app.status.as_ref().expect("a status").tone
}

fn notes_app() -> App {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("older").body("one")).unwrap();
    db.insert_item(&NewItem::note("newer").body("two")).unwrap();
    app_with(db)
}

fn titles(app: &App) -> Vec<&str> {
    app.items.iter().map(|item| item.title.as_str()).collect()
}

#[test]
fn pinning_moves_the_item_to_the_top_and_back() {
    let mut app = notes_app();
    assert_eq!(titles(&app), ["newer", "older"]);

    press(&mut app, KeyCode::Down);
    assert_eq!(app.selected_item().unwrap().title, "older");
    press(&mut app, KeyCode::Char('*'));

    assert_eq!(titles(&app), ["older", "newer"]);
    assert_eq!(app.selected_item().unwrap().title, "older");
    assert!(app.selected_item().unwrap().pinned);
    assert!(app.status.as_ref().unwrap().text.starts_with("Pinned"));

    press(&mut app, KeyCode::Char('*'));
    assert!(!app.selected_item().unwrap().pinned);
    assert!(app.status.as_ref().unwrap().text.starts_with("Unpinned"));
}

#[test]
fn pinning_with_nothing_selected_warns() {
    let mut app = app_with(Database::open_in_memory().unwrap());
    app.update(Action::TogglePin).unwrap();
    assert_eq!(tone(&app), Tone::Warning);
}

#[test]
fn undo_restores_the_deleted_item_exactly() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(
        &NewItem::todo("keep me")
            .body("details")
            .priority(2)
            .tags(["a", "b"]),
    )
    .unwrap();
    let mut app = app_with(db);
    app.update(Action::NextSection).unwrap();
    app.update(Action::ToggleDone).unwrap();
    app.update(Action::TogglePin).unwrap();
    let before = app.selected_item().unwrap().clone();

    press(&mut app, KeyCode::Delete);
    press(&mut app, KeyCode::Char('y'));
    assert!(app.items.is_empty());
    assert!(app.status.as_ref().unwrap().text.contains("u to undo"));
    assert!(app.tags.is_empty());

    press(&mut app, KeyCode::Char('u'));

    assert_eq!(app.selected_item().unwrap(), &before);
    assert_eq!(app.tags.len(), 2);
    assert_eq!(tone(&app), Tone::Success);
    assert!(app.deleted.is_empty());
}

#[test]
fn undo_switches_to_the_section_of_the_restored_item() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::snippet("snip", "sh").body("ls"))
        .unwrap();
    let mut app = app_with(db);
    app.update(Action::PrevSection).unwrap();
    assert_eq!(app.section, Kind::Snippet);

    press(&mut app, KeyCode::Delete);
    press(&mut app, KeyCode::Char('y'));
    app.update(Action::NextSection).unwrap();
    assert_eq!(app.section, Kind::Note);

    app.update(Action::Undo).unwrap();
    assert_eq!(app.section, Kind::Snippet);
    assert_eq!(app.selected_item().unwrap().title, "snip");
}

#[test]
fn undo_walks_back_through_several_deletes() {
    let mut app = notes_app();
    for _ in 0..2 {
        press(&mut app, KeyCode::Delete);
        press(&mut app, KeyCode::Char('y'));
    }
    assert!(app.items.is_empty());

    app.update(Action::Undo).unwrap();
    assert_eq!(titles(&app), ["older"]);
    app.update(Action::Undo).unwrap();
    assert_eq!(app.items.len(), 2);

    app.update(Action::Undo).unwrap();
    assert_eq!(tone(&app), Tone::Warning);
}

#[test]
fn the_undo_history_is_bounded() {
    let mut db = Database::open_in_memory().unwrap();
    for index in 0..UNDO_LIMIT + 5 {
        db.insert_item(&NewItem::note(format!("n{index}"))).unwrap();
    }
    let mut app = app_with(db);
    for _ in 0..UNDO_LIMIT + 5 {
        press(&mut app, KeyCode::Delete);
        press(&mut app, KeyCode::Char('y'));
    }
    assert_eq!(app.deleted.len(), UNDO_LIMIT);
}

#[test]
fn cancelling_a_delete_leaves_nothing_to_undo() {
    let mut app = notes_app();
    press(&mut app, KeyCode::Delete);
    press(&mut app, KeyCode::Char('n'));
    assert!(app.deleted.is_empty());
}

#[test]
fn external_edit_is_requested_once_with_the_right_suffix() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::snippet("snip", "rust").body("fn a() {}"))
        .unwrap();
    let mut app = app_with(db);
    app.update(Action::PrevSection).unwrap();

    press(&mut app, KeyCode::Char('E'));
    let request = app.take_external_edit().expect("a request");

    assert_eq!(request.title, "snip");
    assert_eq!(request.body, "fn a() {}");
    assert_eq!(request.suffix, ".rs");
    assert!(app.take_external_edit().is_none());
}

#[test]
fn external_edit_stores_the_new_body_without_the_editors_final_newline() {
    let mut app = notes_app();
    app.update(Action::ExternalEdit).unwrap();
    let request = app.take_external_edit().unwrap();

    app.finish_external_edit(&request, Ok("rewritten\n".to_string()))
        .unwrap();

    assert_eq!(app.selected_item().unwrap().body, "rewritten");
    assert_eq!(tone(&app), Tone::Success);
}

#[test]
fn external_edit_keeps_a_trailing_newline_the_body_already_had() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("n").body("line\n")).unwrap();
    let mut app = app_with(db);
    app.update(Action::ExternalEdit).unwrap();
    let request = app.take_external_edit().unwrap();

    app.finish_external_edit(&request, Ok("line\nmore\n".to_string()))
        .unwrap();
    assert_eq!(app.selected_item().unwrap().body, "line\nmore\n");
}

#[test]
fn external_edit_without_changes_or_with_errors_leaves_the_item_alone() {
    let mut app = notes_app();
    app.update(Action::ExternalEdit).unwrap();
    let request = app.take_external_edit().unwrap();
    let id = request.id;

    app.finish_external_edit(&request, Ok("two\n".to_string()))
        .unwrap();
    assert_eq!(app.status.as_ref().unwrap().text, "No changes");

    app.finish_external_edit(&request, Err(anyhow::anyhow!("boom")))
        .unwrap();
    assert_eq!(tone(&app), Tone::Danger);
    assert_eq!(app.db.get_item(id).unwrap().unwrap().body, "two");
}

#[test]
fn external_edit_of_an_item_deleted_meanwhile_is_reported() {
    let mut app = notes_app();
    app.update(Action::ExternalEdit).unwrap();
    let request = app.take_external_edit().unwrap();
    app.db.delete_item(request.id).unwrap();

    app.finish_external_edit(&request, Ok("late".to_string()))
        .unwrap();
    assert_eq!(tone(&app), Tone::Warning);
}

#[test]
fn the_theme_is_saved_to_the_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        "# mine\ntheme = \"kanagawa-wave\"  # why not\nicons = \"ascii\"\n",
    )
    .unwrap();

    let mut app = notes_app();
    app.config_path = Some(path.clone());
    press(&mut app, KeyCode::Char('T'));

    assert_eq!(app.theme.name, ThemeName::Dragon);
    assert_eq!(tone(&app), Tone::Success);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        "# mine\ntheme = \"kanagawa-dragon\"  # why not\nicons = \"ascii\"\n"
    );

    app.run_command(Command::CycleTheme).unwrap();
    assert_eq!(Config::load_from(&path).unwrap().theme, ThemeName::Lotus);
}

#[test]
fn without_a_config_path_the_theme_is_session_only() {
    let mut app = notes_app();
    press(&mut app, KeyCode::Char('T'));
    assert!(app.status.as_ref().unwrap().text.contains("this session"));
}

#[test]
fn a_config_that_cannot_be_written_only_warns() {
    let dir = tempfile::tempdir().unwrap();
    // A directory where the file should be: reading it fails.
    let path = dir.path().join("config.toml");
    std::fs::create_dir(&path).unwrap();

    let mut app = notes_app();
    app.config_path = Some(path);
    press(&mut app, KeyCode::Char('T'));

    assert_eq!(tone(&app), Tone::Warning);
    assert_eq!(app.theme.name, ThemeName::Dragon);
}

#[test]
fn help_scrolls_on_narrow_terminals_and_closes_on_other_keys() {
    use quicknotes::app::Mode;

    let mut app = notes_app();
    app.set_area(Rect::new(0, 0, 90, 20));
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.mode, Mode::Help);

    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::PageDown);
    assert_eq!(app.mode, Mode::Help);
    assert_eq!(app.help_scroll, 9);

    press(&mut app, KeyCode::Up);
    assert_eq!(app.help_scroll, 8);
    for _ in 0..100 {
        press(&mut app, KeyCode::PageDown);
    }
    assert!(app.help_scroll <= quicknotes::ui::palette::help_height());

    press(&mut app, KeyCode::Char('x'));
    assert_eq!(app.mode, Mode::Normal);

    // Reopening starts at the top again.
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.help_scroll, 0);
}
