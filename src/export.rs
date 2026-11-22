//! Markdown export: one `.md` file per item, with YAML frontmatter.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::db::{Item, Kind};

/// Renders one item as a Markdown document with frontmatter. Snippets are
/// wrapped in a fenced code block tagged with their language.
pub fn render_item(item: &Item) -> String {
    let mut out = String::from("---\n");
    let _ = writeln!(out, "id: {}", item.id);
    let _ = writeln!(out, "kind: {}", item.kind);
    let _ = writeln!(out, "title: {}", yaml_string(&item.title));
    if let Some(language) = &item.language {
        let _ = writeln!(out, "language: {}", yaml_string(language));
    }
    if item.kind == Kind::Todo {
        let _ = writeln!(out, "done: {}", item.done);
        if let Some(due) = item.due {
            let _ = writeln!(out, "due: {}", due.format("%Y-%m-%d"));
        }
        let _ = writeln!(out, "priority: {}", item.priority);
    }
    if item.pinned {
        out.push_str("pinned: true\n");
    }
    let tags: Vec<String> = item.tags.iter().map(|tag| yaml_string(tag)).collect();
    let _ = writeln!(out, "tags: [{}]", tags.join(", "));
    let _ = writeln!(out, "created: {}", item.created_at.to_rfc3339());
    let _ = writeln!(out, "updated: {}", item.updated_at.to_rfc3339());
    out.push_str("---\n\n");

    let body = item.body.trim_end_matches('\n');
    if item.kind == Kind::Snippet {
        let fence = "`".repeat(longest_backtick_run(body).max(2) + 1);
        let language = item.language.as_deref().unwrap_or("");
        let _ = writeln!(out, "{fence}{language}\n{body}\n{fence}");
    } else if !body.is_empty() {
        let _ = writeln!(out, "{body}");
    }
    out
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

/// Double-quoted YAML scalar.
fn yaml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if other.is_control() => {
                let _ = write!(out, "\\u{:04x}", other as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Lowercase, filesystem-safe slug of a title.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(character);
        } else {
            pending_dash = true;
        }
    }

    let trimmed: String = out.chars().take(50).collect();
    let trimmed = trimmed.trim_end_matches('-');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn file_name(item: &Item) -> String {
    format!("{}-{:04}-{}.md", item.kind, item.id, slug(&item.title))
}

/// Writes every item into `dir` (created when missing) and returns the paths.
pub fn export_to_dir(items: &[Item], dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    items
        .iter()
        .map(|item| {
            let path = dir.join(file_name(item));
            std::fs::write(&path, render_item(item))
                .with_context(|| format!("writing {}", path.display()))?;
            Ok(path)
        })
        .collect()
}

/// All items as one stream: each document is its own frontmatter block.
pub fn render_all(items: &[Item]) -> String {
    items.iter().map(render_item).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, NewItem};

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn slugs_are_filesystem_safe() {
        assert_eq!(slug("Hello, World!"), "hello-world");
        assert_eq!(slug("  --  "), "untitled");
        assert_eq!(slug("a/b\\c:d"), "a-b-c-d");
        assert_eq!(slug("Pagar conta de luz"), "pagar-conta-de-luz");
        assert!(slug(&"x".repeat(200)).len() <= 50);
    }

    #[test]
    fn yaml_strings_escape_specials() {
        assert_eq!(yaml_string("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
        assert_eq!(yaml_string("line\nbreak"), "\"line\\nbreak\"");
        assert_eq!(yaml_string("tab\t"), "\"tab\\t\"");
    }

    #[test]
    fn notes_have_frontmatter_and_a_raw_body() {
        let mut db = db();
        let item = db
            .insert_item(
                &NewItem::note("Ideas: \"big\"")
                    .body("# Heading\n\ntext\n")
                    .tags(["b", "a"]),
            )
            .unwrap();

        let text = render_item(&item);
        assert!(text.starts_with("---\nid: "));
        assert!(text.contains("kind: note\n"));
        assert!(text.contains("title: \"Ideas: \\\"big\\\"\"\n"));
        assert!(text.contains("tags: [\"a\", \"b\"]\n"));
        assert!(!text.contains("done:"));
        assert!(text.ends_with("---\n\n# Heading\n\ntext\n"));
    }

    #[test]
    fn todos_carry_state_due_and_priority() {
        let mut db = db();
        let due = chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let item = db
            .insert_item(&NewItem::todo("Ship").due(due).priority(2))
            .unwrap();
        db.set_done(item.id, true).unwrap();
        db.set_pinned(item.id, true).unwrap();
        let item = db.get_item(item.id).unwrap().unwrap();

        let text = render_item(&item);
        assert!(text.contains("done: true\n"));
        assert!(text.contains("due: 2026-12-31\n"));
        assert!(text.contains("priority: 2\n"));
        assert!(text.contains("pinned: true\n"));
    }

    #[test]
    fn snippets_are_fenced_with_their_language() {
        let mut db = db();
        let item = db
            .insert_item(&NewItem::snippet("fence", "md").body("```rust\nfn a() {}\n```"))
            .unwrap();

        let text = render_item(&item);
        assert!(text.contains("language: \"md\"\n"));
        assert!(text.contains("````md\n```rust\nfn a() {}\n```\n````\n"));
    }

    #[test]
    fn exports_one_file_per_item() {
        let mut db = db();
        let first = db.insert_item(&NewItem::note("First")).unwrap();
        let second = db
            .insert_item(&NewItem::snippet("Second one", "sh").body("ls"))
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out");

        let paths = export_to_dir(&[first.clone(), second.clone()], &target).unwrap();

        assert_eq!(paths.len(), 2);
        assert_eq!(
            file_name(&second),
            format!("snippet-{:04}-second-one.md", second.id)
        );
        for path in &paths {
            assert!(path.exists());
        }
        let written = std::fs::read_to_string(&paths[0]).unwrap();
        assert_eq!(written, render_item(&first));
    }
}
