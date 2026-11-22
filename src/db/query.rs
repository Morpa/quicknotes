//! Queries and mutations for items, tags and full-text search.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};

use super::{Database, Item, Kind, NewItem};

const ITEM_COLUMNS: &str = "i.id, i.kind, i.title, i.body, i.language, i.done, i.due, \
                            i.priority, i.pinned, i.created_at, i.updated_at";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TodoState {
    #[default]
    Any,
    Open,
    Done,
}

/// How list results are ordered. `Smart` is the default: pinned first, open
/// todos before done ones, then due date, priority and recency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Smart,
    Updated,
    Created,
    Title,
    Due,
    Priority,
}

impl SortOrder {
    pub const ALL: [SortOrder; 6] = [
        Self::Smart,
        Self::Updated,
        Self::Created,
        Self::Title,
        Self::Due,
        Self::Priority,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Smart => "smart",
            Self::Updated => "updated",
            Self::Created => "created",
            Self::Title => "title",
            Self::Due => "due",
            Self::Priority => "priority",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|sort| *sort == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    const fn order_by(self) -> &'static str {
        match self {
            Self::Smart => {
                "i.pinned DESC,
                 CASE WHEN i.kind = 'todo' THEN i.done ELSE 0 END ASC,
                 CASE WHEN i.due IS NULL THEN 1 ELSE 0 END ASC,
                 i.due ASC,
                 i.priority DESC,
                 i.updated_at DESC,
                 i.id DESC"
            }
            Self::Updated => "i.pinned DESC, i.updated_at DESC, i.id DESC",
            Self::Created => "i.pinned DESC, i.created_at DESC, i.id DESC",
            Self::Title => "i.pinned DESC, i.title COLLATE NOCASE ASC, i.id ASC",
            Self::Due => {
                "i.pinned DESC,
                 CASE WHEN i.kind = 'todo' THEN i.done ELSE 0 END ASC,
                 CASE WHEN i.due IS NULL THEN 1 ELSE 0 END ASC,
                 i.due ASC,
                 i.updated_at DESC,
                 i.id DESC"
            }
            Self::Priority => {
                "i.pinned DESC,
                 CASE WHEN i.kind = 'todo' THEN i.done ELSE 0 END ASC,
                 i.priority DESC,
                 CASE WHEN i.due IS NULL THEN 1 ELSE 0 END ASC,
                 i.due ASC,
                 i.updated_at DESC,
                 i.id DESC"
            }
        }
    }
}

/// Filters shared by list views, the CLI and the search palette.
#[derive(Debug, Clone, Default)]
pub struct ItemQuery {
    pub kind: Option<Kind>,
    pub tag: Option<String>,
    pub todo_state: TodoState,
    pub search: Option<String>,
    pub pinned_only: bool,
    pub sort: SortOrder,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub notes: u64,
    pub open_todos: u64,
    pub done_todos: u64,
    pub snippets: u64,
}

impl Database {
    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("reading schema version")
    }

    pub fn stats(&self) -> Result<Stats> {
        let (notes, open_todos, done_todos, snippets) = self.conn.query_row(
            "SELECT
                 COALESCE(SUM(kind = 'note'), 0),
                 COALESCE(SUM(kind = 'todo' AND done = 0), 0),
                 COALESCE(SUM(kind = 'todo' AND done = 1), 0),
                 COALESCE(SUM(kind = 'snippet'), 0)
             FROM items",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )?;

        Ok(Stats {
            notes: notes as u64,
            open_todos: open_todos as u64,
            done_todos: done_todos as u64,
            snippets: snippets as u64,
        })
    }

    pub fn count_items(&self, kind: Kind) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM items WHERE kind = ?1",
                [kind],
                |row| row.get(0),
            )
            .context("counting items")?;
        Ok(count as u64)
    }

    pub fn count_open_todos(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM items WHERE kind = 'todo' AND done = 0",
                [],
                |row| row.get(0),
            )
            .context("counting open todos")?;
        Ok(count as u64)
    }

    pub fn get_item(&self, id: i64) -> Result<Option<Item>> {
        let sql = format!("SELECT {ITEM_COLUMNS} FROM items i WHERE i.id = ?1");
        let mut stmt = self.conn.prepare(&sql).context("preparing item lookup")?;
        let found = stmt
            .query_row([id], row_to_item)
            .optional()
            .context("loading item")?;

        let Some(item) = found else {
            return Ok(None);
        };

        let mut items = vec![item];
        self.attach_tags(&mut items)?;
        Ok(items.pop())
    }

    pub fn list_items(&self, query: &ItemQuery) -> Result<Vec<Item>> {
        let mut sql = format!("SELECT {ITEM_COLUMNS} FROM items i");
        let mut conditions: Vec<&str> = Vec::new();
        let mut values: Vec<Value> = Vec::new();

        if let Some(tag) = query
            .tag
            .as_deref()
            .map(normalize_tag)
            .filter(|tag| !tag.is_empty())
        {
            sql.push_str(" JOIN item_tags it ON it.item_id = i.id JOIN tags t ON t.id = it.tag_id");
            conditions.push("t.name = ?");
            values.push(Value::Text(tag));
        }

        if let Some(search) = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|search| !search.is_empty())
        {
            sql.push_str(" JOIN items_fts f ON f.rowid = i.id");
            conditions.push("items_fts MATCH ?");
            values.push(Value::Text(fts_query(search)));
        }

        if let Some(kind) = query.kind {
            conditions.push("i.kind = ?");
            values.push(Value::Text(kind.as_str().to_string()));
        }

        match query.todo_state {
            TodoState::Any => {}
            TodoState::Open => conditions.push("(i.kind != 'todo' OR i.done = 0)"),
            TodoState::Done => conditions.push("(i.kind != 'todo' OR i.done = 1)"),
        }

        if query.pinned_only {
            conditions.push("i.pinned = 1");
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY ");
        sql.push_str(query.sort.order_by());

        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT ?");
            values.push(Value::Integer(limit as i64));
        }

        let mut stmt = self.conn.prepare(&sql).context("preparing item query")?;
        let rows = stmt.query_map(params_from_iter(values), row_to_item)?;
        let mut items: Vec<Item> = rows.collect::<rusqlite::Result<_>>()?;
        drop(stmt);

        self.attach_tags(&mut items)?;
        Ok(items)
    }

    pub fn insert_item(&mut self, new: &NewItem) -> Result<Item> {
        let title = new.title.trim();
        if title.is_empty() {
            bail!("item title cannot be empty");
        }

        let language = clean_language(new.language.as_deref());
        let now = Utc::now();

        let tx = self.conn.transaction().context("starting insert")?;
        tx.execute(
            "INSERT INTO items
                 (kind, title, body, language, done, due, priority, pinned, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, 0, ?7, ?7)",
            params![
                new.kind,
                title,
                new.body,
                language,
                new.due,
                new.priority.min(2),
                now,
            ],
        )
        .context("inserting item")?;

        let id = tx.last_insert_rowid();
        sync_tags(&tx, id, &new.tags)?;
        tx.commit().context("committing insert")?;

        self.get_item(id)?
            .with_context(|| format!("inserted item {id} could not be read back"))
    }

    pub fn update_item(&mut self, item: &Item) -> Result<()> {
        let title = item.title.trim();
        if title.is_empty() {
            bail!("item title cannot be empty");
        }

        let language = clean_language(item.language.as_deref());

        let tx = self.conn.transaction().context("starting update")?;
        let changed = tx
            .execute(
                "UPDATE items
                 SET title = ?1, body = ?2, language = ?3, done = ?4, due = ?5,
                     priority = ?6, pinned = ?7, updated_at = ?8
                 WHERE id = ?9",
                params![
                    title,
                    item.body,
                    language,
                    item.done,
                    item.due,
                    item.priority.min(2),
                    item.pinned,
                    Utc::now(),
                    item.id,
                ],
            )
            .context("updating item")?;

        if changed == 0 {
            bail!("item {} does not exist", item.id);
        }

        sync_tags(&tx, item.id, &item.tags)?;
        tx.commit().context("committing update")?;
        Ok(())
    }

    pub fn delete_item(&mut self, id: i64) -> Result<bool> {
        let tx = self.conn.transaction().context("starting delete")?;
        let deleted = tx
            .execute("DELETE FROM items WHERE id = ?1", [id])
            .context("deleting item")?
            > 0;
        if deleted {
            prune_tags(&tx)?;
        }
        tx.commit().context("committing delete")?;
        Ok(deleted)
    }

    /// Marks a todo as done/undone. Returns the new state.
    pub fn set_done(&mut self, id: i64, done: bool) -> Result<bool> {
        let changed = self
            .conn
            .execute(
                "UPDATE items SET done = ?1, updated_at = ?2
                 WHERE id = ?3 AND kind = 'todo'",
                params![done, Utc::now(), id],
            )
            .context("updating todo state")?;

        if changed == 0 {
            bail!("todo {id} does not exist");
        }
        Ok(done)
    }

    /// Flips the done state of a todo and returns the new state.
    pub fn toggle_done(&mut self, id: i64) -> Result<bool> {
        let current: Option<bool> = self
            .conn
            .query_row(
                "SELECT done FROM items WHERE id = ?1 AND kind = 'todo'",
                [id],
                |row| row.get(0),
            )
            .optional()
            .context("reading todo state")?;

        match current {
            Some(done) => self.set_done(id, !done),
            None => bail!("todo {id} does not exist"),
        }
    }

    /// Re-inserts a previously deleted item with its original id, timestamps,
    /// state and tags. Fails when the id is taken again.
    pub fn restore_item(&mut self, item: &Item) -> Result<Item> {
        let tx = self.conn.transaction().context("starting restore")?;
        tx.execute(
            "INSERT INTO items
                 (id, kind, title, body, language, done, due, priority, pinned,
                  created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                item.id,
                item.kind,
                item.title,
                item.body,
                clean_language(item.language.as_deref()),
                item.done,
                item.due,
                item.priority.min(2),
                item.pinned,
                item.created_at,
                item.updated_at,
            ],
        )
        .context("restoring item")?;
        sync_tags(&tx, item.id, &item.tags)?;
        tx.commit().context("committing restore")?;

        self.get_item(item.id)?
            .with_context(|| format!("restored item {} could not be read back", item.id))
    }

    /// Sets the priority (clamped to 0..=2) and returns the stored value.
    pub fn set_priority(&mut self, id: i64, priority: u8) -> Result<u8> {
        let priority = priority.min(2);
        let changed = self
            .conn
            .execute(
                "UPDATE items SET priority = ?1, updated_at = ?2 WHERE id = ?3",
                params![priority, Utc::now(), id],
            )
            .context("updating priority")?;

        if changed == 0 {
            bail!("item {id} does not exist");
        }
        Ok(priority)
    }

    pub fn set_due(&mut self, id: i64, due: Option<NaiveDate>) -> Result<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE items SET due = ?1, updated_at = ?2 WHERE id = ?3",
                params![due, Utc::now(), id],
            )
            .context("updating due date")?;

        if changed == 0 {
            bail!("item {id} does not exist");
        }
        Ok(())
    }

    pub fn set_pinned(&mut self, id: i64, pinned: bool) -> Result<()> {
        let changed = self
            .conn
            .execute(
                "UPDATE items SET pinned = ?1, updated_at = ?2 WHERE id = ?3",
                params![pinned, Utc::now(), id],
            )
            .context("updating pinned state")?;

        if changed == 0 {
            bail!("item {id} does not exist");
        }
        Ok(())
    }

    /// Tags with the number of items that use them, sorted by name.
    pub fn all_tags(&self) -> Result<Vec<(String, u64)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.name, COUNT(it.item_id)
                 FROM tags t
                 JOIN item_tags it ON it.tag_id = t.id
                 GROUP BY t.name
                 ORDER BY t.name",
            )
            .context("preparing tag query")?;

        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((name, count as u64))
        })?;

        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn attach_tags(&self, items: &mut [Item]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let placeholders = vec!["?"; items.len()].join(",");
        let sql = format!(
            "SELECT it.item_id, t.name
             FROM item_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE it.item_id IN ({placeholders})
             ORDER BY t.name"
        );

        let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
        let mut stmt = self.conn.prepare(&sql).context("preparing tag lookup")?;
        let mut rows = stmt.query(params_from_iter(ids.iter()))?;

        let mut tags: HashMap<i64, Vec<String>> = HashMap::new();
        while let Some(row) = rows.next()? {
            tags.entry(row.get(0)?).or_default().push(row.get(1)?);
        }

        for item in items {
            item.tags = tags.remove(&item.id).unwrap_or_default();
        }
        Ok(())
    }
}

fn row_to_item(row: &Row<'_>) -> rusqlite::Result<Item> {
    Ok(Item {
        id: row.get("id")?,
        kind: row.get("kind")?,
        title: row.get("title")?,
        body: row.get("body")?,
        language: row.get("language")?,
        done: row.get("done")?,
        due: row.get("due")?,
        priority: row.get("priority")?,
        pinned: row.get("pinned")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        tags: Vec::new(),
    })
}

fn sync_tags(conn: &Connection, item_id: i64, tags: &[String]) -> Result<()> {
    conn.execute("DELETE FROM item_tags WHERE item_id = ?1", [item_id])
        .context("clearing item tags")?;

    for tag in normalize_tags(tags) {
        conn.execute(
            "INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
            [&tag],
        )
        .context("inserting tag")?;
        conn.execute(
            "INSERT OR IGNORE INTO item_tags (item_id, tag_id)
             SELECT ?1, id FROM tags WHERE name = ?2",
            params![item_id, &tag],
        )
        .context("linking tag")?;
    }

    prune_tags(conn)
}

fn prune_tags(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM item_tags)",
        [],
    )
    .context("pruning unused tags")?;
    Ok(())
}

fn clean_language(language: Option<&str>) -> Option<String> {
    language
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
}

/// Normalizes a single tag: trims, drops a leading `#` and lowercases.
pub fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches('#').to_lowercase()
}

/// Normalizes, sorts and deduplicates tags.
pub fn normalize_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut tags: Vec<String> = tags
        .into_iter()
        .map(|tag| normalize_tag(tag.as_ref()))
        .filter(|tag| !tag.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

/// Turns free-form input into a safe FTS5 query where every term is a prefix.
pub fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}
