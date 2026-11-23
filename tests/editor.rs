use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use quicknotes::app::{AUTOSAVE_DELAY, App, Mode, Tone};
use quicknotes::config::Config;
use quicknotes::db::{Database, Kind, NewItem};
use quicknotes::event::Action;
use quicknotes::ui::editor::Field;
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

#[test]
fn creates_a_note_through_the_editor() {
    let mut app = app_with_notes();

    app.update(Action::New).unwrap();
    assert_eq!(app.mode, Mode::Edit);
    assert!(app.editor.is_some());

    type_text(&mut app, "Groceries");
    press(&mut app, KeyCode::Tab);
    type_text(&mut app, "milk and eggs");
    press_ctrl(&mut app, 's');

    assert_eq!(app.mode, Mode::Normal);
    assert!(app.editor.is_none());
    assert_eq!(app.items.len(), 3);

    let selected = app.selected_item().unwrap();
    assert_eq!(selected.title, "Groceries");
    assert_eq!(selected.body, "milk and eggs");
    assert_eq!(selected.kind, Kind::Note);
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Success)
    );
}

#[test]
fn enter_moves_from_title_to_body() {
    let mut app = app_with_notes();
    app.update(Action::New).unwrap();

    type_text(&mut app, "Title");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.editor.as_ref().unwrap().field, Field::Body);

    type_text(&mut app, "body text");
    press_ctrl(&mut app, 's');

    let selected = app.selected_item().unwrap();
    assert_eq!(selected.title, "Title");
    assert_eq!(selected.body, "body text");
}

#[test]
fn empty_titles_are_rejected_in_the_editor() {
    let mut app = app_with_notes();
    app.update(Action::New).unwrap();

    press(&mut app, KeyCode::Tab);
    type_text(&mut app, "body without a title");
    press_ctrl(&mut app, 's');

    assert_eq!(app.mode, Mode::Edit);
    assert_eq!(app.items.len(), 2);
    assert_eq!(
        app.editor
            .as_ref()
            .and_then(|editor| editor.error.as_deref()),
        Some("Title cannot be empty")
    );
}

#[test]
fn edits_an_existing_item() {
    let mut app = app_with_notes();
    let id = app.selected_item().unwrap().id;

    app.update(Action::Edit).unwrap();
    assert_eq!(app.editor.as_ref().unwrap().item_id, Some(id));

    type_text(&mut app, " updated");
    press_ctrl(&mut app, 's');

    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.items.len(), 2);
    let saved = app.db.get_item(id).unwrap().unwrap();
    assert!(saved.title.ends_with(" updated"));
}

#[test]
fn deletes_after_confirmation() {
    let mut app = app_with_notes();
    let id = app.selected_item().unwrap().id;

    app.update(Action::Delete).unwrap();
    assert_eq!(app.mode, Mode::ConfirmDelete);
    assert_eq!(app.pending_delete.as_ref().unwrap().id, id);

    press(&mut app, KeyCode::Char('y'));

    assert_eq!(app.mode, Mode::Normal);
    assert!(app.pending_delete.is_none());
    assert_eq!(app.items.len(), 1);
    assert!(app.db.get_item(id).unwrap().is_none());
}

#[test]
fn cancel_returns_to_the_editor_then_discards() {
    let mut app = app_with_notes();
    app.update(Action::New).unwrap();
    type_text(&mut app, "draft");

    press(&mut app, KeyCode::Esc);
    assert_eq!(app.mode, Mode::ConfirmDiscard);

    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.mode, Mode::Edit);
    assert_eq!(app.editor.as_ref().unwrap().title_text(), "draft");

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('y'));

    assert_eq!(app.mode, Mode::Normal);
    assert!(app.editor.is_none());
    assert_eq!(app.items.len(), 2);
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Normal)
    );
}

#[test]
fn autosaved_new_items_are_removed_when_discarded() {
    let mut app = app_with_notes();
    app.update(Action::New).unwrap();
    type_text(&mut app, "Autosaved note");

    app.editor.as_mut().unwrap().last_edit = Instant::now() - AUTOSAVE_DELAY;
    app.tick();

    let id = app.editor.as_ref().unwrap().item_id.unwrap();
    assert!(app.db.get_item(id).unwrap().is_some());
    assert_eq!(app.items.len(), 2);

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('y'));

    assert!(app.db.get_item(id).unwrap().is_none());
    assert_eq!(app.items.len(), 2);
}

#[test]
fn autosaved_edits_are_restored_when_discarded() {
    let mut app = app_with_notes();
    let id = app.selected_item().unwrap().id;
    let original = app.selected_item().unwrap().title.clone();

    app.update(Action::Edit).unwrap();
    type_text(&mut app, " changed");

    app.editor.as_mut().unwrap().last_edit = Instant::now() - AUTOSAVE_DELAY;
    app.tick();

    assert!(
        app.db
            .get_item(id)
            .unwrap()
            .unwrap()
            .title
            .ends_with(" changed")
    );

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('y'));

    assert_eq!(app.db.get_item(id).unwrap().unwrap().title, original);
}

#[test]
fn snippet_language_is_kept() {
    let mut app = app_with_notes();
    app.update(Action::NextSection).unwrap();
    app.update(Action::NextSection).unwrap();
    assert_eq!(app.section, Kind::Snippet);

    app.update(Action::New).unwrap();
    type_text(&mut app, "entry api");
    press(&mut app, KeyCode::Tab);
    type_text(&mut app, "RUST");
    press(&mut app, KeyCode::Tab);
    type_text(&mut app, "map.entry(key)");
    press_ctrl(&mut app, 's');

    let snippet = app.selected_item().unwrap();
    assert_eq!(snippet.kind, Kind::Snippet);
    assert_eq!(snippet.language.as_deref(), Some("rust"));
    assert_eq!(snippet.body, "map.entry(key)");
}

#[test]
fn normal_mode_keys_open_and_delete() {
    let mut app = app_with_notes();

    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.mode, Mode::Edit);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.mode, Mode::Normal);

    press(&mut app, KeyCode::Enter);
    assert_eq!(app.mode, Mode::Edit);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.mode, Mode::Normal);

    press(&mut app, KeyCode::Delete);
    assert_eq!(app.mode, Mode::ConfirmDelete);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn ctrl_p_toggles_the_split_preview() {
    let mut app = app_with_notes();
    app.update(Action::New).unwrap();
    assert!(!app.editor.as_ref().unwrap().split);

    press_ctrl(&mut app, 'p');
    assert!(app.editor.as_ref().unwrap().split);

    press_ctrl(&mut app, 'p');
    assert!(!app.editor.as_ref().unwrap().split);
}

fn open_new_note(app: &mut App) {
    app.update(Action::New).unwrap();
    press(app, KeyCode::Tab);
    assert_eq!(app.editor.as_ref().unwrap().field, Field::Body);
}

#[test]
fn bracketed_paste_keeps_lines_and_tabs_in_the_body() {
    let mut app = app_with_notes();
    open_new_note(&mut app);

    app.handle_paste("one\r\n\ttwo\nthree").unwrap();

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.body_text(), "one\n\ttwo\nthree");
    assert_eq!(editor.field, Field::Body, "a paste never switches fields");
    assert!(editor.dirty);
}

#[test]
fn paste_into_the_title_is_flattened() {
    let mut app = app_with_notes();
    app.update(Action::New).unwrap();

    app.handle_paste("line one\n\tline two").unwrap();

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.title_text(), "line one line two");
    assert_eq!(editor.field, Field::Title);
}

#[test]
fn paste_uses_clipboard_markdown_when_it_matches_the_pasted_text() {
    use quicknotes::clipboard::Pasted;

    let mut app = app_with_notes();
    open_new_note(&mut app);
    app.paste_source = Box::new(|| {
        Ok(Pasted {
            text: "Hello world".into(),
            markdown: Some("Hello **world**".into()),
        })
    });

    app.handle_paste("Hello world").unwrap();
    assert_eq!(app.editor.as_ref().unwrap().body_text(), "Hello **world**");

    // Text from elsewhere (not what the clipboard holds) is left untouched.
    app.handle_paste(" other").unwrap();
    assert_eq!(
        app.editor.as_ref().unwrap().body_text(),
        "Hello **world** other"
    );
}

#[test]
fn ctrl_v_pastes_the_clipboard_as_markdown() {
    use quicknotes::clipboard::Pasted;

    let mut app = app_with_notes();
    open_new_note(&mut app);
    app.paste_source = Box::new(|| {
        Ok(Pasted {
            text: "- a b".into(),
            markdown: Some("- a\n- b".into()),
        })
    });

    press_ctrl(&mut app, 'v');
    assert_eq!(app.editor.as_ref().unwrap().body_text(), "- a\n- b");

    app.paste_source = Box::new(|| {
        Ok(Pasted {
            text: "plain".into(),
            markdown: None,
        })
    });
    press_ctrl(&mut app, 'v');
    assert_eq!(app.editor.as_ref().unwrap().body_text(), "- a\n- bplain");
}

#[test]
fn ctrl_v_reports_an_unreadable_clipboard() {
    let mut app = app_with_notes();
    open_new_note(&mut app);
    app.paste_source = Box::new(|| Err(anyhow::anyhow!("no display")));

    press_ctrl(&mut app, 'v');

    assert_eq!(app.status.as_ref().unwrap().tone, Tone::Warning);
    assert_eq!(app.editor.as_ref().unwrap().body_text(), "");
}

#[test]
fn editing_a_multiline_body_shows_one_buffer_line_per_line() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("Multi").body("one\ntwo\n\nthree\n"))
        .unwrap();
    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));

    app.update(Action::Edit).unwrap();

    let editor = app.editor.as_ref().unwrap();
    assert_eq!(editor.body_line_count(), 5);
    assert_eq!(editor.body_text(), "one\ntwo\n\nthree\n");
}
