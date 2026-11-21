use anyhow::Result;
use quicknotes::{config::Config, db::Database, logging, paths, ui::theme::Theme};

fn main() -> Result<()> {
    let data_dir = paths::data_dir()?;
    let _log_guard = logging::init(&data_dir)?;

    let config_path = paths::config_file()?;
    if Config::ensure_file(&config_path)? {
        tracing::info!(path = %config_path.display(), "wrote default configuration");
    }

    let config = Config::load();
    let database_path = paths::database_file(&config)?;
    let db = Database::open(&database_path)?;
    let theme = Theme::from_config(&config);
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
    println!();
    println!("Foundation ready. The interactive TUI arrives in phase 2.");

    Ok(())
}
