//! `qn doctor`: checks the installation and the terminal.

use std::fmt;
use std::path::Path;

use crate::config::{ColorMode, Config, Icons};
use crate::db::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Info,
    Warn,
    Fail,
}

impl Level {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Ok => "✓",
            Self::Info => "·",
            Self::Warn => "!",
            Self::Fail => "✗",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub level: Level,
    pub name: &'static str,
    pub detail: String,
}

impl Check {
    fn new(level: Level, name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            level,
            name,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "  {} {:<10} {}",
            self.level.symbol(),
            self.name,
            self.detail
        )
    }
}

/// What the terminal environment looks like; injected so tests need no tty.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub term: Option<String>,
    pub colorterm: Option<String>,
    pub locale: Option<String>,
    pub size: Option<(u16, u16)>,
    pub interactive: bool,
}

impl Environment {
    pub fn from_process() -> Self {
        use std::io::IsTerminal;

        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        Self {
            term: var("TERM"),
            colorterm: var("COLORTERM"),
            locale: var("LC_ALL")
                .or_else(|| var("LC_CTYPE"))
                .or_else(|| var("LANG")),
            size: crossterm::terminal::size().ok(),
            interactive: std::io::stdout().is_terminal(),
        }
    }
}

pub struct Context<'a> {
    pub config_path: &'a Path,
    pub config: &'a Config,
    pub database_path: &'a Path,
    pub data_dir: &'a Path,
    pub environment: &'a Environment,
}

pub fn run(context: &Context<'_>) -> Vec<Check> {
    let mut checks = vec![Check::new(
        Level::Info,
        "version",
        format!("quicknotes {}", env!("CARGO_PKG_VERSION")),
    )];
    checks.push(check_config(context.config_path));
    checks.push(check_data_dir(context.data_dir));
    checks.extend(check_database(context.database_path));
    checks.extend(check_terminal(context.environment, context.config));
    checks.push(check_font(context.config));
    checks.push(check_clipboard());

    let overrides: Vec<&str> = ["QUICKNOTES_CONFIG", "QUICKNOTES_DB", "QUICKNOTES_LOG"]
        .into_iter()
        .filter(|name| std::env::var_os(name).is_some())
        .collect();
    if !overrides.is_empty() {
        checks.push(Check::new(
            Level::Info,
            "overrides",
            format!("environment: {}", overrides.join(", ")),
        ));
    }
    checks
}

fn check_config(path: &Path) -> Check {
    if !path.exists() {
        return Check::new(
            Level::Warn,
            "config",
            format!(
                "{} not found (defaults in use; run `qn setup`)",
                path.display()
            ),
        );
    }
    match Config::load_from(path) {
        Ok(config) => Check::new(
            Level::Ok,
            "config",
            format!(
                "{} · {} · {}",
                path.display(),
                config.theme,
                config.background.as_str()
            ),
        ),
        Err(error) => Check::new(Level::Fail, "config", format!("{error:#}")),
    }
}

fn check_data_dir(dir: &Path) -> Check {
    if let Err(error) = std::fs::create_dir_all(dir) {
        return Check::new(
            Level::Fail,
            "data dir",
            format!("{}: {error}", dir.display()),
        );
    }

    let probe = dir.join(".qn-doctor");
    match std::fs::write(&probe, b"ok").and_then(|()| std::fs::remove_file(&probe)) {
        Ok(()) => Check::new(
            Level::Ok,
            "data dir",
            format!("{} is writable", dir.display()),
        ),
        Err(error) => Check::new(
            Level::Fail,
            "data dir",
            format!("{} is not writable: {error}", dir.display()),
        ),
    }
}

fn check_database(path: &Path) -> Vec<Check> {
    if !path.exists() {
        return vec![Check::new(
            Level::Info,
            "database",
            format!("{} will be created on first run", path.display()),
        )];
    }

    let db = match Database::open(path) {
        Ok(db) => db,
        Err(error) => return vec![Check::new(Level::Fail, "database", format!("{error:#}"))],
    };

    let mut checks = Vec::new();
    let summary = match (db.schema_version(), db.stats()) {
        (Ok(version), Ok(stats)) => Check::new(
            Level::Ok,
            "database",
            format!(
                "{} · schema v{version} · {} notes, {} open todos, {} snippets",
                path.display(),
                stats.notes,
                stats.open_todos,
                stats.snippets
            ),
        ),
        (Err(error), _) | (_, Err(error)) => {
            Check::new(Level::Fail, "database", format!("{error:#}"))
        }
    };
    checks.push(summary);

    let integrity: rusqlite::Result<String> =
        db.conn()
            .query_row("PRAGMA integrity_check", [], |row| row.get(0));
    checks.push(match integrity {
        Ok(result) if result == "ok" => {
            Check::new(Level::Ok, "integrity", "SQLite integrity check passed")
        }
        Ok(result) => Check::new(Level::Fail, "integrity", result),
        Err(error) => Check::new(Level::Fail, "integrity", error.to_string()),
    });

    let fts = db.conn().execute(
        "INSERT INTO items_fts (items_fts, rank) VALUES ('integrity-check', 1)",
        [],
    );
    checks.push(match fts {
        Ok(_) => Check::new(Level::Ok, "search", "FTS5 index is consistent"),
        Err(error) => Check::new(
            Level::Fail,
            "search",
            format!("FTS5 index is out of sync ({error}); rebuild it with: INSERT INTO items_fts(items_fts) VALUES('rebuild')"),
        ),
    });
    checks
}

fn check_terminal(environment: &Environment, config: &Config) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(match environment.term.as_deref() {
        Some("dumb") | None => Check::new(
            Level::Warn,
            "terminal",
            "TERM is unset or `dumb`; the interface needs a real terminal",
        ),
        Some(term) => Check::new(Level::Ok, "terminal", format!("TERM={term}")),
    });

    let truecolor = environment
        .colorterm
        .as_deref()
        .is_some_and(|value| value.contains("truecolor") || value.contains("24bit"));
    checks.push(match (truecolor, config.color_mode) {
        (true, _) => Check::new(Level::Ok, "colors", "truecolor supported"),
        (false, ColorMode::Ansi256) => Check::new(Level::Ok, "colors", "color_mode = \"256\""),
        (false, ColorMode::Truecolor) => Check::new(
            Level::Warn,
            "colors",
            "COLORTERM does not advertise truecolor; if colors look wrong set color_mode = \"256\"",
        ),
    });

    checks.push(match environment.size {
        Some((width, height)) if width >= 80 && height >= 24 => {
            Check::new(Level::Ok, "size", format!("{width}x{height}"))
        }
        Some((width, height)) => Check::new(
            Level::Warn,
            "size",
            format!(
                "{width}x{height}: panes hide below 80 columns, 24 rows is the comfortable minimum"
            ),
        ),
        None if !environment.interactive => {
            Check::new(Level::Info, "size", "not attached to a terminal")
        }
        None => Check::new(Level::Warn, "size", "could not read the terminal size"),
    });

    checks.push(match environment.locale.as_deref() {
        Some(locale) if locale.to_uppercase().replace('-', "").contains("UTF8") => {
            Check::new(Level::Ok, "locale", locale)
        }
        other => Check::new(
            Level::Warn,
            "locale",
            format!(
                "{} is not UTF-8; box drawing and icons may break",
                other.unwrap_or("unset")
            ),
        ),
    });

    checks
}

fn check_font(config: &Config) -> Check {
    match config.icons {
        Icons::Ascii => Check::new(
            Level::Ok,
            "font",
            "icons = \"ascii\"; no special font needed",
        ),
        Icons::Nerd => Check::new(
            Level::Info,
            "font",
            "󰈙 󰄱 󰅴  ← you should see three icons; a Nerd Font cannot be detected from here. \
             Use Iosevka Nerd Font Propo, or set icons = \"ascii\"",
        ),
    }
}

fn check_clipboard() -> Check {
    match arboard::Clipboard::new() {
        Ok(_) => Check::new(Level::Ok, "clipboard", "system clipboard available"),
        Err(error) => Check::new(
            Level::Warn,
            "clipboard",
            format!("`y` (copy) will not work: {error}"),
        ),
    }
}

pub fn has_failures(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.level == Level::Fail)
}

pub fn format_report(checks: &[Check]) -> String {
    let mut out = String::from("quicknotes doctor\n");
    for check in checks {
        out.push_str(&check.to_string());
        out.push('\n');
    }

    let fails = checks.iter().filter(|c| c.level == Level::Fail).count();
    let warns = checks.iter().filter(|c| c.level == Level::Warn).count();
    out.push_str(&match (fails, warns) {
        (0, 0) => "\nAll good.\n".to_string(),
        (0, warns) => format!("\n{warns} warning(s), nothing blocking.\n"),
        (fails, warns) => format!("\n{fails} problem(s), {warns} warning(s).\n"),
    });
    out
}
