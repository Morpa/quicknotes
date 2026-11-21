//! quicknotes — local-first notes, todos and code snippets for the terminal.
//!
//! Phase 1 provides the foundation: configuration, SQLite storage and the
//! Kanagawa color theme. The interactive TUI arrives in phase 2.

pub mod config;
pub mod db;
pub mod logging;
pub mod paths;
pub mod ui;

pub use config::Config;
pub use db::{Database, Item, ItemQuery, Kind, NewItem, TodoState};
