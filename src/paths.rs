//! Platform paths for configuration, data and the SQLite database.
//!
//! Every path can be overridden through the environment:
//! `QUICKNOTES_CONFIG`, `QUICKNOTES_DB` and `QUICKNOTES_LOG`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::config::Config;

const QUALIFIER: &str = "dev";
const ORGANIZATION: &str = "andremorpa";
const APPLICATION: &str = "quicknotes";

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .context("could not determine application directories for this platform")
}

/// Directory that holds the database and the log file.
pub fn data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

/// Location of `config.toml`, honouring `QUICKNOTES_CONFIG`.
pub fn config_file() -> Result<PathBuf> {
    if let Some(path) = env_path("QUICKNOTES_CONFIG") {
        return Ok(path);
    }
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

/// Default database location when the config does not set `db_path`.
pub fn default_database_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("quicknotes.db"))
}

/// Database location: `QUICKNOTES_DB` > `config.db_path` > default data dir.
pub fn database_file(config: &Config) -> Result<PathBuf> {
    if let Some(path) = env_path("QUICKNOTES_DB") {
        return Ok(path);
    }
    if let Some(path) = config.db_path.as_deref() {
        return Ok(expand_tilde(path));
    }
    default_database_file()
}

/// Expands a leading `~` to the user's home directory.
pub fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(rest) = path.strip_prefix("~")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    path.to_path_buf()
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_paths_are_untouched() {
        let path = Path::new("/tmp/quicknotes.db");
        assert_eq!(expand_tilde(path), path);
    }

    #[test]
    fn relative_paths_are_untouched() {
        let path = Path::new("notes/quicknotes.db");
        assert_eq!(expand_tilde(path), path);
    }
}
