//! quicknotes — local-first notes, todos and code snippets for the terminal.

pub mod app;
pub mod config;
pub mod db;
pub mod event;
pub mod logging;
pub mod markdown;
pub mod paths;
pub mod tui;
pub mod ui;

pub use app::App;
pub use config::Config;
pub use db::{Database, Item, ItemQuery, Kind, NewItem, TodoState};
