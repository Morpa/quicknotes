//! File logging. The TUI owns the terminal, so logs must never go to stderr.

use std::path::Path;

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Initializes logging into `<data_dir>/quicknotes.log`.
///
/// The returned guard must stay alive for as long as logging is needed.
/// Verbosity can be controlled with `QUICKNOTES_LOG` (e.g. `debug`).
pub fn init(data_dir: &Path) -> Result<WorkerGuard> {
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("creating data directory {}", data_dir.display()))?;

    let appender = tracing_appender::rolling::never(data_dir, "quicknotes.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter =
        EnvFilter::try_from_env("QUICKNOTES_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(false)
        .try_init()
        .map_err(|error| anyhow::anyhow!("initializing tracing: {error}"))?;

    Ok(guard)
}
