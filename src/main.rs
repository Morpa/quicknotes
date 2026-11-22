use anyhow::Result;
use quicknotes::{
    app::App,
    config::{Config, ThemeName},
    db::Database,
    logging, osc, paths, tui,
    ui::theme::Theme,
};
use std::io::IsTerminal;

fn main() -> Result<()> {
    open_interface()
}

fn open_interface() -> Result<()> {
    let data_dir = paths::data_dir()?;
    let _log_guard = logging::init(&data_dir)?;

    let config_path = paths::config_file()?;
    if Config::ensure_file(&config_path)? {
        tracing::info!(path = %config_path.display(), "wrote default configuration");
    }

    let interactive = std::io::stdout().is_terminal();
    let mut config = Config::load();
    // Only `theme = "auto"` is worth a round trip to the terminal.
    if interactive && config.theme == ThemeName::Auto {
        config = config.resolve_auto(osc::terminal_is_dark());
    }

    let database_path = paths::database_file(&config)?;
    let db = Database::open(&database_path)?;
    let theme = Theme::from_config(&config);

    if !interactive {
        print_summary(&config, &db, &theme, &config_path)?;
        return Ok(());
    }

    let mut app = App::new(config, theme, db)?;
    app.config_path = Some(config_path);
    tui::run(app)
}

fn print_summary(
    config: &Config,
    db: &Database,
    theme: &Theme,
    config_path: &std::path::Path,
) -> Result<()> {
    let stats = db.stats()?;

    println!("quicknotes {}", env!("CARGO_PKG_VERSION"));
    println!("  config    {}", config_path.display());
    println!("  database  {}", db.path().display());
    println!(
        "  theme     {} · {} · {}",
        theme.name,
        config.background.as_str(),
        config.accent.as_str()
    );
    println!(
        "  items     {} notes · {} open todos · {} done · {} snippets",
        stats.notes, stats.open_todos, stats.done_todos, stats.snippets
    );

    Ok(())
}
