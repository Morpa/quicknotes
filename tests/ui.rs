use quicknotes::app::App;
use quicknotes::config::Config;
use quicknotes::db::{Database, NewItem};
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
    assert!(text.contains("TAGS"));
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
    assert!(!text.contains("TAGS"));
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
