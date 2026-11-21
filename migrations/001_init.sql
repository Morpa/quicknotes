-- quicknotes schema v1
-- One table for every item; `kind` distinguishes notes, todos and snippets.

CREATE TABLE items (
    id         INTEGER PRIMARY KEY,
    kind       TEXT    NOT NULL CHECK (kind IN ('note', 'todo', 'snippet')),
    title      TEXT    NOT NULL,
    body       TEXT    NOT NULL DEFAULT '',
    language   TEXT,
    done       INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
    due        TEXT,
    priority   INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 2),
    pinned     INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    created_at TEXT    NOT NULL,
    updated_at TEXT    NOT NULL
);

CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE item_tags (
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, tag_id)
);

CREATE INDEX idx_items_kind ON items (kind);
CREATE INDEX idx_items_done ON items (done);
CREATE INDEX idx_items_due ON items (due);
CREATE INDEX idx_items_updated ON items (updated_at DESC);
CREATE INDEX idx_item_tags_tag ON item_tags (tag_id);

-- Full-text search over titles and bodies. External content keeps a single
-- source of truth; the triggers below mirror inserts, updates and deletes.
CREATE VIRTUAL TABLE items_fts USING fts5(
    title,
    body,
    content = 'items',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER items_after_insert AFTER INSERT ON items BEGIN
    INSERT INTO items_fts (rowid, title, body)
    VALUES (new.id, new.title, new.body);
END;

CREATE TRIGGER items_after_delete AFTER DELETE ON items BEGIN
    INSERT INTO items_fts (items_fts, rowid, title, body)
    VALUES ('delete', old.id, old.title, old.body);
END;

CREATE TRIGGER items_after_update AFTER UPDATE ON items BEGIN
    INSERT INTO items_fts (items_fts, rowid, title, body)
    VALUES ('delete', old.id, old.title, old.body);
    INSERT INTO items_fts (rowid, title, body)
    VALUES (new.id, new.title, new.body);
END;
