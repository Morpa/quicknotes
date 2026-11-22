//! Core data types: notes, todos and snippets share one `Item` shape.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Note,
    Todo,
    Snippet,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Note, Kind::Todo, Kind::Snippet];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Todo => "todo",
            Self::Snippet => "snippet",
        }
    }

    /// Plural label used by the sidebar.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Note => "notes",
            Self::Todo => "todos",
            Self::Snippet => "snippets",
        }
    }

    pub const fn singular(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Todo => "todo",
            Self::Snippet => "snippet",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Note => Self::Todo,
            Self::Todo => Self::Snippet,
            Self::Snippet => Self::Note,
        }
    }

    pub const fn prev(self) -> Self {
        match self {
            Self::Note => Self::Snippet,
            Self::Todo => Self::Note,
            Self::Snippet => Self::Todo,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("unknown item kind `{0}` (expected note, todo or snippet)")]
    UnknownKind(String),
}

impl FromStr for Kind {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "note" => Ok(Self::Note),
            "todo" => Ok(Self::Todo),
            "snippet" => Ok(Self::Snippet),
            other => Err(ModelError::UnknownKind(other.to_string())),
        }
    }
}

impl ToSql for Kind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for Kind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let raw = value.as_str()?;
        raw.parse()
            .map_err(|error: ModelError| FromSqlError::Other(Box::new(error)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: i64,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    pub language: Option<String>,
    pub done: bool,
    pub due: Option<NaiveDate>,
    pub priority: u8,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Item {
    pub fn is_open_todo(&self) -> bool {
        self.kind == Kind::Todo && !self.done
    }

    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        self.is_open_todo() && self.due.is_some_and(|due| due < today)
    }
}

/// Data required to create an item. Built with the fluent helpers below.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub kind: Kind,
    pub title: String,
    pub body: String,
    pub language: Option<String>,
    pub due: Option<NaiveDate>,
    pub priority: u8,
}

impl NewItem {
    pub fn new(kind: Kind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            body: String::new(),
            language: None,
            due: None,
            priority: 0,
        }
    }

    pub fn note(title: impl Into<String>) -> Self {
        Self::new(Kind::Note, title)
    }

    pub fn todo(title: impl Into<String>) -> Self {
        Self::new(Kind::Todo, title)
    }

    pub fn snippet(title: impl Into<String>, language: impl Into<String>) -> Self {
        let mut item = Self::new(Kind::Snippet, title);
        item.language = Some(language.into());
        item
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn due(mut self, due: NaiveDate) -> Self {
        self.due = Some(due);
        self
    }

    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}
