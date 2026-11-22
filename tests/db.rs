use chrono::{NaiveDate, Utc};
use quicknotes::db::{Database, ItemQuery, Kind, NewItem, Stats, TodoState};

fn memory_db() -> Database {
    Database::open_in_memory().expect("in-memory database")
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

#[test]
fn migrates_to_the_latest_schema() {
    let db = memory_db();
    assert_eq!(db.schema_version().unwrap(), 1);
}

#[test]
fn insert_and_get_roundtrip() {
    let mut db = memory_db();
    let due = date(2026, 12, 1);

    let item = db
        .insert_item(
            &NewItem::todo("Pagar boleto")
                .body("conta de luz")
                .due(due)
                .priority(2),
        )
        .unwrap();

    assert_eq!(item.kind, Kind::Todo);
    assert_eq!(item.title, "Pagar boleto");
    assert_eq!(item.body, "conta de luz");
    assert_eq!(item.due, Some(due));
    assert_eq!(item.priority, 2);
    assert!(!item.done);
    assert!(!item.pinned);
    assert!((Utc::now() - item.created_at).num_seconds().abs() < 5);

    let fetched = db.get_item(item.id).unwrap().expect("item exists");
    assert_eq!(fetched, item);
}

#[test]
fn empty_titles_are_rejected() {
    let mut db = memory_db();
    assert!(db.insert_item(&NewItem::note("   ")).is_err());
}

#[test]
fn priority_is_clamped_and_language_normalized() {
    let mut db = memory_db();
    let todo = db
        .insert_item(&NewItem::todo("urgent").priority(9))
        .unwrap();
    assert_eq!(todo.priority, 2);

    let snippet = db
        .insert_item(&NewItem::snippet("entry api", "  Rust "))
        .unwrap();
    assert_eq!(snippet.language.as_deref(), Some("rust"));
}

#[test]
fn update_replaces_fields() {
    let mut db = memory_db();
    let item = db.insert_item(&NewItem::note("first")).unwrap();

    let mut edited = item.clone();
    edited.title = "second".to_string();
    edited.body = "new body".to_string();
    db.update_item(&edited).unwrap();

    let fetched = db.get_item(item.id).unwrap().unwrap();
    assert_eq!(fetched.title, "second");
    assert_eq!(fetched.body, "new body");
    assert!(fetched.updated_at >= item.updated_at);
}

#[test]
fn delete_removes_the_item() {
    let mut db = memory_db();
    let item = db.insert_item(&NewItem::note("gone")).unwrap();

    assert!(db.delete_item(item.id).unwrap());
    assert!(!db.delete_item(item.id).unwrap());
    assert!(db.get_item(item.id).unwrap().is_none());
}

#[test]
fn toggling_a_todo_returns_the_new_state() {
    let mut db = memory_db();
    let todo = db.insert_item(&NewItem::todo("toggle me")).unwrap();

    assert!(db.toggle_done(todo.id).unwrap());
    assert!(db.get_item(todo.id).unwrap().unwrap().done);

    assert!(!db.toggle_done(todo.id).unwrap());
    assert!(!db.get_item(todo.id).unwrap().unwrap().done);

    assert!(db.toggle_done(9_999).is_err());
}

#[test]
fn filters_and_ordering() {
    let mut db = memory_db();
    let pinned = db.insert_item(&NewItem::note("pinned")).unwrap();
    db.set_pinned(pinned.id, true).unwrap();
    db.insert_item(&NewItem::note("regular")).unwrap();

    let done = db.insert_item(&NewItem::todo("finished")).unwrap();
    db.set_done(done.id, true).unwrap();
    let open = db
        .insert_item(&NewItem::todo("pending").due(date(2026, 12, 1)))
        .unwrap();

    let items = db.list_items(&ItemQuery::default()).unwrap();
    assert_eq!(items[0].title, "pinned");

    let open_position = items.iter().position(|item| item.id == open.id).unwrap();
    let done_position = items.iter().position(|item| item.id == done.id).unwrap();
    assert!(open_position < done_position);

    let open_todos = db
        .list_items(&ItemQuery {
            kind: Some(Kind::Todo),
            todo_state: TodoState::Open,
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(open_todos.len(), 1);
    assert_eq!(open_todos[0].id, open.id);

    let done_todos = db
        .list_items(&ItemQuery {
            kind: Some(Kind::Todo),
            todo_state: TodoState::Done,
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(done_todos.len(), 1);
    assert_eq!(done_todos[0].id, done.id);

    let limited = db
        .list_items(&ItemQuery {
            limit: Some(1),
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(limited.len(), 1);
}

#[test]
fn full_text_search_matches_titles_and_bodies() {
    let mut db = memory_db();
    let note = db
        .insert_item(&NewItem::note("Rust async runtime").body("tokio and futures"))
        .unwrap();
    db.insert_item(&NewItem::note("Meeting notes")).unwrap();

    let hits = db
        .list_items(&ItemQuery {
            search: Some("async".to_string()),
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, note.id);

    // Prefix matching.
    let hits = db
        .list_items(&ItemQuery {
            search: Some("futur".to_string()),
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);

    // Special characters never break the query.
    let hits = db
        .list_items(&ItemQuery {
            search: Some("ru\"st".to_string()),
            ..ItemQuery::default()
        })
        .unwrap();
    assert!(hits.is_empty());

    // Updates are mirrored into the FTS index.
    let mut updated = note.clone();
    updated.body.push_str(" zebra");
    db.update_item(&updated).unwrap();
    let hits = db
        .list_items(&ItemQuery {
            search: Some("zebra".to_string()),
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
}

#[test]
fn stats_count_every_kind() {
    let mut db = memory_db();
    db.insert_item(&NewItem::note("n1")).unwrap();
    db.insert_item(&NewItem::note("n2")).unwrap();
    db.insert_item(&NewItem::snippet("s1", "rust")).unwrap();
    let done = db.insert_item(&NewItem::todo("t1")).unwrap();
    db.insert_item(&NewItem::todo("t2")).unwrap();
    db.set_done(done.id, true).unwrap();

    assert_eq!(
        db.stats().unwrap(),
        Stats {
            notes: 2,
            open_todos: 1,
            done_todos: 1,
            snippets: 1,
        }
    );
}

#[test]
fn data_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quicknotes.db");

    {
        let mut db = Database::open(&path).unwrap();
        db.insert_item(&NewItem::note("persisted")).unwrap();
    }

    let db = Database::open(&path).unwrap();
    assert_eq!(db.count_items(Kind::Note).unwrap(), 1);
    assert_eq!(db.schema_version().unwrap(), 1);
}

#[test]
fn sort_orders_and_priority_updates() {
    use quicknotes::db::SortOrder;

    let mut db = Database::open_in_memory().unwrap();
    let low = db.insert_item(&NewItem::todo("b low")).unwrap();
    let high = db.insert_item(&NewItem::todo("a high")).unwrap();

    assert_eq!(db.set_priority(high.id, 9).unwrap(), 2);
    assert!(db.set_priority(9_999, 1).is_err());

    fn titles(db: &Database, sort: SortOrder) -> Vec<String> {
        db.list_items(&ItemQuery {
            kind: Some(Kind::Todo),
            sort,
            ..ItemQuery::default()
        })
        .unwrap()
        .into_iter()
        .map(|item| item.title)
        .collect()
    }

    assert_eq!(titles(&db, SortOrder::Title), ["a high", "b low"]);
    assert_eq!(titles(&db, SortOrder::Priority), ["a high", "b low"]);
    assert_eq!(titles(&db, SortOrder::Created), ["a high", "b low"]);

    db.set_done(high.id, true).unwrap();
    assert_eq!(titles(&db, SortOrder::Priority), ["b low", "a high"]);

    let due = chrono::NaiveDate::from_ymd_opt(2030, 1, 2).unwrap();
    db.set_due(low.id, Some(due)).unwrap();
    assert_eq!(db.get_item(low.id).unwrap().unwrap().due, Some(due));
    db.set_due(low.id, None).unwrap();
    assert_eq!(db.get_item(low.id).unwrap().unwrap().due, None);
    assert!(db.set_due(9_999, None).is_err());

    assert_eq!(SortOrder::Priority.next(), SortOrder::Smart);
}

#[test]
fn restore_brings_back_a_deleted_item_unchanged() {
    let mut db = memory_db();
    let original = db
        .insert_item(
            &NewItem::todo("Pagar luz")
                .body("conta")
                .priority(2)
                .due(date(2026, 12, 1)),
        )
        .unwrap();
    db.set_pinned(original.id, true).unwrap();
    db.set_done(original.id, true).unwrap();
    let original = db.get_item(original.id).unwrap().unwrap();

    assert!(db.delete_item(original.id).unwrap());
    assert!(db.get_item(original.id).unwrap().is_none());

    let restored = db.restore_item(&original).unwrap();

    assert_eq!(restored, original);
    // The search index follows the restore.
    let found = db
        .list_items(&ItemQuery {
            search: Some("conta".into()),
            ..ItemQuery::default()
        })
        .unwrap();
    assert_eq!(found.len(), 1);

    // Restoring over a live row is an error, not a duplicate.
    assert!(db.restore_item(&original).is_err());
}
