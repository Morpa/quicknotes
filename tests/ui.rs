use quicknotes::app::App;
use quicknotes::config::Config;
use quicknotes::db::{Database, NewItem};
use quicknotes::event::Action;
use quicknotes::ui::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn build_app() -> App {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("Async runtime").body("tokio and futures"))
        .unwrap();
    db.insert_item(&NewItem::snippet("entry api", "rust").body("map.entry(key)"))
        .unwrap();

    let config = Config::default();
    let theme = Theme::from_config(&config);
    App::new(config, theme, db).unwrap()
}

fn render(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
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
fn wide_screen_shows_all_panes() {
    let mut app = build_app();
    app.set_area(Rect::new(0, 0, 120, 32));

    let text = render(&app, 120, 32);

    assert!(text.contains("QUICKNOTES"));
    assert!(text.contains("NOTES"));
    assert!(text.contains("Async runtime"));
    assert!(text.contains("tokio and futures"));
    assert!(text.contains("move"));
}

#[test]
fn narrow_screen_hides_side_panes() {
    let mut app = build_app();
    app.set_area(Rect::new(0, 0, 40, 20));

    let text = render(&app, 40, 20);

    assert!(text.contains("QUICKNOTES"));
    assert!(text.contains("Async runtime"));
}

#[test]
fn empty_section_shows_a_hint() {
    let mut app = build_app();
    app.set_area(Rect::new(0, 0, 120, 32));

    // Snippets exist, notes exist; switch twice to todos (empty).
    app.section = quicknotes::db::Kind::Todo;
    app.items.clear();

    let text = render(&app, 120, 32);
    assert!(text.contains("Nothing here yet"));
}

#[test]
fn editor_overlay_shows_fields_and_hints() {
    let mut app = build_app();
    app.set_area(Rect::new(0, 0, 120, 32));

    app.update(Action::New).unwrap();
    let text = render(&app, 120, 32);

    assert!(text.contains("NEW NOTE"));
    assert!(text.contains("TITLE"));
    assert!(text.contains("BODY"));
    assert!(text.contains("ctrl+s"));
    assert!(text.contains("cancel"));
}

#[test]
fn split_preview_renders_markdown() {
    let mut app = build_app();
    app.set_area(Rect::new(0, 0, 120, 32));

    app.update(Action::New).unwrap();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('p'),
        crossterm::event::KeyModifiers::CONTROL,
    ))
    .unwrap();

    let text = render(&app, 120, 32);
    assert!(text.contains("PREVIEW"));
    assert!(text.contains("TITLE"));
}

#[test]
fn preview_renders_markdown_instead_of_raw_text() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::note("Async runtime").body("# Heading\n\nsome **bold** text"))
        .unwrap();

    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));

    let text = render(&app, 120, 32);

    assert!(text.contains("Heading"));
    assert!(!text.contains("# Heading"));
    assert!(text.contains("bold"));
}

#[test]
fn delete_asks_for_confirmation() {
    let mut app = build_app();
    app.set_area(Rect::new(0, 0, 120, 32));

    app.update(Action::Delete).unwrap();
    let text = render(&app, 120, 32);

    assert!(text.contains("Delete"));
    assert!(text.contains("y confirm"));
}

fn buffer(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| quicknotes::ui::draw(frame, app))
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn snippet_preview_is_syntax_highlighted() {
    use ratatui::style::Color;

    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::snippet("main", "rust").body("fn main() {\n    let x = 1;\n}"))
        .unwrap();
    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));
    app.update(Action::PrevSection).unwrap();

    let buffer = buffer(&app, 120, 32);
    let violet = Color::Rgb(0x95, 0x7F, 0xB8);
    let keyword_cells = buffer
        .content()
        .iter()
        .filter(|cell| cell.fg == violet && !cell.symbol().trim().is_empty())
        .count();

    // `fn` and `let` are keywords.
    assert!(keyword_cells >= 5, "{keyword_cells}");
}

#[test]
fn todo_rows_show_priority_and_sort_label() {
    let mut db = Database::open_in_memory().unwrap();
    db.insert_item(&NewItem::todo("urgent thing").priority(2))
        .unwrap();
    let config = Config::default();
    let theme = Theme::from_config(&config);
    let mut app = App::new(config, theme, db).unwrap();
    app.set_area(Rect::new(0, 0, 120, 32));
    app.update(Action::NextSection).unwrap();
    app.update(Action::CycleSort).unwrap();

    let text = render(&app, 120, 32);
    assert!(text.contains("!!"));
    assert!(text.contains("by updated"));
}
