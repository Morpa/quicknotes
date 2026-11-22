use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::Result;
use quicknotes::cli::{self, Cli, ExportArgs, ExportTarget};
use quicknotes::db::ItemQuery;
use quicknotes::doctor::{self, Environment};
use quicknotes::{
    app::App,
    config::{Config, ThemeName},
    db::Database,
    export, logging, osc, paths, tui,
    ui::theme::Theme,
};

fn main() -> ExitCode {
    let command = match cli::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("qn: {message}\n\n{}", cli::HELP);
            return ExitCode::from(2);
        }
    };

    match run(command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("qn: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Cli) -> Result<ExitCode> {
    match command {
        Cli::Help => print!("{}", cli::HELP),
        Cli::Version => println!("qn {}", env!("CARGO_PKG_VERSION")),
        Cli::Setup { print_config } => setup(print_config)?,
        Cli::Doctor => return doctor(),
        Cli::Export(args) => export(&args)?,
        Cli::Tui => open_interface()?,
    }
    Ok(ExitCode::SUCCESS)
}

fn setup(print_config: bool) -> Result<()> {
    if print_config {
        print!("{}", Config::default().to_toml()?);
        return Ok(());
    }

    let path = paths::config_file()?;
    if Config::ensure_file(&path)? {
        println!("wrote {}", path.display());
    } else {
        println!("{} already exists; left untouched", path.display());
    }
    println!(
        "database: {}",
        paths::database_file(&Config::load())?.display()
    );
    println!("`qn setup --print-config` prints the default configuration");
    Ok(())
}

fn doctor() -> Result<ExitCode> {
    let config_path = paths::config_file()?;
    let config = Config::load();
    let database_path = paths::database_file(&config)?;
    let data_dir = paths::data_dir()?;
    let environment = Environment::from_process();

    let checks = doctor::run(&doctor::Context {
        config_path: &config_path,
        config: &config,
        database_path: &database_path,
        data_dir: &data_dir,
        environment: &environment,
    });
    print!("{}", doctor::format_report(&checks));

    Ok(if doctor::has_failures(&checks) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn export(args: &ExportArgs) -> Result<()> {
    let config = Config::load();
    let db = Database::open(paths::database_file(&config)?)?;
    let items = db.list_items(&ItemQuery {
        kind: args.kind,
        tag: args.tag.clone(),
        ..ItemQuery::default()
    })?;

    match &args.target {
        ExportTarget::Stdout => print!("{}", export::render_all(&items)),
        ExportTarget::Dir(dir) => {
            let written = export::export_to_dir(&items, dir)?;
            println!("exported {} item(s) to {}", written.len(), dir.display());
        }
    }
    Ok(())
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

    let app = App::new(config, theme, db)?;
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
