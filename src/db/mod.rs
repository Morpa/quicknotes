//! SQLite storage. One connection per process, WAL mode, versioned migrations.

mod model;
mod query;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

pub use model::{Item, Kind, ModelError, NewItem};
pub use query::{ItemQuery, SortOrder, Stats, TodoState, fts_query};

const MIGRATIONS: &[&str] = &[include_str!("../../migrations/001_init.sql")];

pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        Self::configure(conn, path.to_path_buf())
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory database")?;
        Self::configure(conn, PathBuf::from(":memory:"))
    }

    fn configure(conn: Connection, path: PathBuf) -> Result<Self> {
        conn.busy_timeout(Duration::from_millis(5_000))
            .context("configuring busy timeout")?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .context("enabling foreign keys")?;

        if path != Path::new(":memory:") {
            let mode: String = conn
                .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
                .context("enabling WAL")?;
            if !mode.eq_ignore_ascii_case("wal") {
                tracing::warn!(mode, "database did not switch to WAL mode");
            }
            conn.pragma_update(None, "synchronous", "NORMAL")
                .context("setting synchronous mode")?;
        }

        let mut db = Self { conn, path };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    fn migrate(&mut self) -> Result<()> {
        let current: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("reading schema version")?;

        if current < 0 || current as usize > MIGRATIONS.len() {
            bail!(
                "database schema version {current} is not supported by this build (latest is {})",
                MIGRATIONS.len()
            );
        }

        for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let version = (index + 1) as i64;
            let tx = self.conn.transaction().context("starting migration")?;
            tx.execute_batch(sql)
                .with_context(|| format!("applying migration {version}"))?;
            tx.pragma_update(None, "user_version", version)
                .context("recording schema version")?;
            tx.commit().context("committing migration")?;
            tracing::info!(version, "applied database migration");
        }

        Ok(())
    }
}
