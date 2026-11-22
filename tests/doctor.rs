use quicknotes::config::Config;
use quicknotes::db::{Database, NewItem};
use quicknotes::doctor::{Context, Environment, Level, format_report, has_failures, run};

fn good_environment() -> Environment {
    Environment {
        term: Some("xterm-256color".into()),
        colorterm: Some("truecolor".into()),
        locale: Some("en_US.UTF-8".into()),
        size: Some((120, 40)),
        interactive: true,
    }
}

fn level_of(checks: &[quicknotes::doctor::Check], name: &str) -> Level {
    checks
        .iter()
        .find(|check| check.name == name)
        .unwrap_or_else(|| panic!("no `{name}` check"))
        .level
}

#[test]
fn healthy_setup_has_no_failures() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    Config::ensure_file(&config_path).unwrap();
    let database_path = dir.path().join("qn.db");
    let mut db = Database::open(&database_path).unwrap();
    db.insert_item(&NewItem::note("hello").body("world"))
        .unwrap();
    drop(db);

    let config = Config::load_from(&config_path).unwrap();
    let environment = good_environment();
    let checks = run(&Context {
        config_path: &config_path,
        config: &config,
        database_path: &database_path,
        data_dir: dir.path(),
        environment: &environment,
    });

    assert!(!has_failures(&checks), "{}", format_report(&checks));
    assert_eq!(level_of(&checks, "config"), Level::Ok);
    assert_eq!(level_of(&checks, "database"), Level::Ok);
    assert_eq!(level_of(&checks, "integrity"), Level::Ok);
    assert_eq!(level_of(&checks, "search"), Level::Ok);
    assert_eq!(level_of(&checks, "colors"), Level::Ok);
    assert_eq!(level_of(&checks, "size"), Level::Ok);
    assert_eq!(level_of(&checks, "locale"), Level::Ok);

    let report = format_report(&checks);
    assert!(report.starts_with("quicknotes doctor"));
    assert!(report.contains("1 notes"));
}

#[test]
fn missing_files_are_not_failures() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let environment = good_environment();
    let checks = run(&Context {
        config_path: &dir.path().join("missing.toml"),
        config: &config,
        database_path: &dir.path().join("missing.db"),
        data_dir: dir.path(),
        environment: &environment,
    });

    assert_eq!(level_of(&checks, "config"), Level::Warn);
    assert_eq!(level_of(&checks, "database"), Level::Info);
    assert!(!has_failures(&checks));
    // Checking must not create the database.
    assert!(!dir.path().join("missing.db").exists());
}

#[test]
fn broken_config_and_database_fail() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "theme = \"neon\"").unwrap();
    let database_path = dir.path().join("qn.db");
    std::fs::write(&database_path, b"this is not sqlite, not even close at all").unwrap();

    let config = Config::default();
    let environment = good_environment();
    let checks = run(&Context {
        config_path: &config_path,
        config: &config,
        database_path: &database_path,
        data_dir: dir.path(),
        environment: &environment,
    });

    assert_eq!(level_of(&checks, "config"), Level::Fail);
    assert_eq!(level_of(&checks, "database"), Level::Fail);
    assert!(has_failures(&checks));
    assert!(format_report(&checks).contains("problem(s)"));
}

#[test]
fn a_desynchronized_search_index_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let database_path = dir.path().join("qn.db");
    let db = Database::open(&database_path).unwrap();
    // Add a row without the trigger that keeps the index in sync.
    db.conn()
        .execute_batch(
            "DROP TRIGGER items_after_insert;
             INSERT INTO items (kind, title, body, created_at, updated_at)
             VALUES ('note', 'unindexed', 'text', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
        )
        .unwrap();
    drop(db);

    let config = Config::default();
    let environment = good_environment();
    let checks = run(&Context {
        config_path: &dir.path().join("config.toml"),
        config: &config,
        database_path: &database_path,
        data_dir: dir.path(),
        environment: &environment,
    });

    assert_eq!(level_of(&checks, "search"), Level::Fail);
}

#[test]
fn terminal_problems_are_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let environment = Environment {
        term: Some("dumb".into()),
        colorterm: None,
        locale: Some("C".into()),
        size: Some((60, 15)),
        interactive: true,
    };
    let checks = run(&Context {
        config_path: &dir.path().join("config.toml"),
        config: &config,
        database_path: &dir.path().join("qn.db"),
        data_dir: dir.path(),
        environment: &environment,
    });

    for name in ["terminal", "colors", "size", "locale"] {
        assert_eq!(level_of(&checks, name), Level::Warn, "{name}");
    }
    assert!(!has_failures(&checks));
}

#[test]
fn ascii_icons_need_no_font() {
    let dir = tempfile::tempdir().unwrap();
    let config: Config = toml::from_str("icons = \"ascii\"\ncolor_mode = \"256\"").unwrap();
    let environment = Environment::default();
    let checks = run(&Context {
        config_path: &dir.path().join("config.toml"),
        config: &config,
        database_path: &dir.path().join("qn.db"),
        data_dir: dir.path(),
        environment: &environment,
    });

    assert_eq!(level_of(&checks, "font"), Level::Ok);
    assert_eq!(level_of(&checks, "colors"), Level::Ok);
}
