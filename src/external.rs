//! Editing an item's body in the user's own editor (`$VISUAL` / `$EDITOR`).

use std::io::Write;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::db::Kind;

/// `$VISUAL`, then `$EDITOR`, then `vi`.
pub fn editor_command() -> String {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string())
}

/// File extension for the temporary file, so the editor picks a syntax.
pub fn extension(kind: Kind, language: Option<&str>) -> String {
    if kind != Kind::Snippet {
        return ".md".to_string();
    }

    let language = language.unwrap_or("").trim().to_lowercase();
    let extension = match language.as_str() {
        "" => "txt",
        "rust" => "rs",
        "python" => "py",
        "javascript" => "js",
        "typescript" => "ts",
        "shell" | "bash" | "zsh" | "fish" => "sh",
        "markdown" => "md",
        "golang" => "go",
        "c++" => "cpp",
        "c#" | "csharp" => "cs",
        other if other.chars().all(|c| c.is_ascii_alphanumeric()) => other,
        _ => "txt",
    };
    format!(".{extension}")
}

/// Writes `text` to a temporary file, runs `command <file>` and returns the
/// file's contents afterwards. `command` may carry arguments (`code --wait`);
/// it is split on whitespace, without shell quoting.
pub fn edit_text(command: &str, text: &str, suffix: &str) -> Result<String> {
    let mut parts = command.split_whitespace();
    let Some(program) = parts.next() else {
        bail!("no editor configured; set $EDITOR");
    };

    let mut file = tempfile::Builder::new()
        .prefix("quicknotes-")
        .suffix(suffix)
        .tempfile()
        .context("creating a temporary file")?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .context("writing the temporary file")?;

    let status = Command::new(program)
        .args(parts)
        .arg(file.path())
        .status()
        .with_context(|| format!("starting `{program}`"))?;
    if !status.success() {
        bail!("`{program}` exited with {status}");
    }

    std::fs::read_to_string(file.path()).context("reading the edited file")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_follow_the_language() {
        assert_eq!(extension(Kind::Note, None), ".md");
        assert_eq!(extension(Kind::Todo, Some("rust")), ".md");
        assert_eq!(extension(Kind::Snippet, Some("Rust")), ".rs");
        assert_eq!(extension(Kind::Snippet, Some("bash")), ".sh");
        assert_eq!(extension(Kind::Snippet, Some("zig")), ".zig");
        assert_eq!(extension(Kind::Snippet, None), ".txt");
        assert_eq!(extension(Kind::Snippet, Some("../x")), ".txt");
    }

    #[test]
    fn an_empty_command_is_an_error() {
        assert!(edit_text("   ", "x", ".md").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn runs_the_editor_and_returns_the_result() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-editor.sh");
        std::fs::write(&script, "#!/bin/sh\nprintf 'edited ' >> \"$1\"\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = edit_text(script.to_str().unwrap(), "body", ".md").unwrap();
        assert_eq!(result, "bodyedited ");
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_editor_is_reported() {
        assert!(edit_text("false", "x", ".md").is_err());
        assert!(edit_text("/definitely/not/an/editor", "x", ".md").is_err());
    }
}
