use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use quicknotes::app::{App, Mode, Tone};
use quicknotes::config::Config;
use quicknotes::db::{Database, Kind, NewItem, SortOrder};
use quicknotes::event::Action;
use quicknotes::ui::palette::Command;
use quicknotes::ui::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn app() -> App {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(
        &NewItem::note("Tokio runtime")
            .body("async executor")
            .tags(["rust"]),
    )
    .unwrap();
    db.insert_item(
        &NewItem::note("Token bucket")
            .body("rate limiting")
            .tags(["algo"]),
    )
    .unwrap();
    db.insert_item(&NewItem::note("Groceries").body("milk").tags(["rust"]))
        .unwrap();
    db.insert_item(&NewItem::todo("Read tokio docs")).unwrap();

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

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        press(app, KeyCode::Char(character));
    }
}

fn titles(app: &App) -> Vec<&str> {
    app.items.iter().map(|item| item.title.as_str()).collect()
}

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal
        .draw(|frame| quicknotes::ui::draw(frame, app))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn search_narrows_as_you_type_and_matches_prefixes() {
    let mut app = app();
    assert_eq!(app.items.len(), 3);

    press(&mut app, KeyCode::Char('/'));
    assert_eq!(app.mode, Mode::Search);

    type_text(&mut app, "tok");
    assert_eq!(app.items.len(), 2);
    assert!(titles(&app).contains(&"Tokio runtime"));

    type_text(&mut app, "io");
    assert_eq!(titles(&app), ["Tokio runtime"]);

    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.items.len(), 2);
}

#[test]
fn search_matches_bodies() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "rate");
    assert_eq!(titles(&app), ["Token bucket"]);
}

#[test]
fn enter_keeps_the_search_and_esc_clears_it_before_quitting() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "tok");
    press(&mut app, KeyCode::Enter);

    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.items.len(), 2);
    assert!(app.list_summary().contains("/tok"));

    press(&mut app, KeyCode::Esc);
    assert!(!app.should_quit);
    assert!(app.search.is_empty());
    assert_eq!(app.items.len(), 3);

    press(&mut app, KeyCode::Esc);
    assert!(app.should_quit);
}

#[test]
fn esc_inside_the_bar_cancels_the_search() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "tok");
    press(&mut app, KeyCode::Esc);

    assert_eq!(app.mode, Mode::Normal);
    assert!(app.search.is_empty());
    assert_eq!(app.items.len(), 3);
}

#[test]
fn section_tag_and_query_combine() {
    let mut app = app();
    app.tag_filter = Some("rust".to_string());
    press(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "tok");
    assert_eq!(titles(&app), ["Tokio runtime"]);

    press(&mut app, KeyCode::Enter);
    app.update(Action::NextSection).unwrap();
    assert_eq!(app.section, Kind::Todo);
    // No todo carries the tag, so the combined filter finds nothing.
    assert!(app.items.is_empty());
    assert!(screen(&app).contains("No matches"));

    app.update(Action::Back).unwrap();
    app.update(Action::Back).unwrap();
    assert_eq!(titles(&app), ["Read tokio docs"]);
}

#[test]
fn special_characters_never_break_the_query() {
    let mut app = app();
    press(&mut app, KeyCode::Char('/'));
    for input in ["\"", "tok*", "a OR", "(", "-x", "NEAR("] {
        type_text(&mut app, input);
        assert_eq!(app.mode, Mode::Search, "{input}");
        app.update(Action::Refresh).unwrap();
        press(&mut app, KeyCode::Char('u'));
        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
            .unwrap();
    }
}

#[test]
fn palette_runs_the_best_fuzzy_match() {
    let mut app = app();
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.mode, Mode::Palette);

    type_text(&mut app, "srt");
    assert_eq!(
        app.palette.as_ref().unwrap().current(),
        Some(Command::CycleSort)
    );
    press(&mut app, KeyCode::Enter);

    assert_eq!(app.mode, Mode::Normal);
    assert!(app.palette.is_none());
    assert_eq!(app.sort, SortOrder::Updated);
}

#[test]
fn palette_navigates_sections_and_opens_other_overlays() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "todos");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.section, Kind::Todo);

    app.run_command(Command::Search).unwrap();
    assert_eq!(app.mode, Mode::Search);
    press(&mut app, KeyCode::Esc);

    app.run_command(Command::Help).unwrap();
    assert_eq!(app.mode, Mode::Help);
}

#[test]
fn palette_closes_with_esc_and_ignores_empty_results() {
    let mut app = app();
    press(&mut app, KeyCode::Char(':'));
    type_text(&mut app, "qqqq");
    assert!(app.palette.as_ref().unwrap().rows.is_empty());
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.mode, Mode::Normal);
    assert!(!app.should_quit);

    press(&mut app, KeyCode::Char(':'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn clear_filters_command_resets_search_and_tag() {
    let mut app = app();
    app.tag_filter = Some("rust".to_string());
    app.search = "tok".to_string();
    app.run_command(Command::ClearFilters).unwrap();

    assert!(app.search.is_empty());
    assert_eq!(app.tag_filter, None);
    assert_eq!(app.items.len(), 3);
    assert_eq!(
        app.status.as_ref().map(|status| status.tone),
        Some(Tone::Normal)
    );
}

#[test]
fn help_overlay_opens_with_question_mark_and_any_key_closes_it() {
    let mut app = app();
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.mode, Mode::Help);
    let text = screen(&app);
    assert!(text.contains("HELP"));
    assert!(text.contains("command palette"));

    press(&mut app, KeyCode::Char('z'));
    assert_eq!(app.mode, Mode::Normal);
    assert!(!app.should_quit);
}

#[test]
fn overlays_render_on_a_small_terminal() {
    let mut app = app();
    app.set_area(Rect::new(0, 0, 40, 12));
    for key in ['?', 'x'] {
        press(&mut app, KeyCode::Char(key));
    }
    press(&mut app, KeyCode::Char(':'));
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| quicknotes::ui::draw(frame, &app))
        .unwrap();
}
