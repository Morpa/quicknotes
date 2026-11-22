use std::path::Path;
use std::process::{Command, Output};

fn qn(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(args)
        .env("QUICKNOTES_DB", dir.join("qn.db"))
        .env("QUICKNOTES_CONFIG", dir.join("config.toml"))
        .env("QUICKNOTES_LOG", "off")
        .env_remove("COLORFGBG")
        .current_dir(dir)
        .output()
        .expect("run qn")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn seed(dir: &Path) {
    use quicknotes::db::{Database, NewItem};

    let mut db = Database::open(dir.join("qn.db")).unwrap();
    db.insert_item(
        &NewItem::note("Plan")
            .body("# Plan\n\nship it")
            .tags(["work"]),
    )
    .unwrap();
    db.insert_item(
        &NewItem::snippet("Hello", "rust")
            .body("fn main() {}")
            .tags(["rust"]),
    )
    .unwrap();
    db.insert_item(&NewItem::todo("Call mom")).unwrap();
}

#[test]
fn version_and_help() {
    let dir = tempfile::tempdir().unwrap();

    let version = qn(dir.path(), &["--version"]);
    assert!(version.status.success());
    assert!(stdout(&version).starts_with("qn "));

    let help = qn(dir.path(), &["--help"]);
    assert!(help.status.success());
    assert!(stdout(&help).contains("qn doctor"));
}

#[test]
fn unknown_commands_exit_with_status_two() {
    let dir = tempfile::tempdir().unwrap();
    let output = qn(dir.path(), &["frobnicate"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown command"));
}

#[test]
fn setup_prints_and_writes_the_config() {
    let dir = tempfile::tempdir().unwrap();

    let printed = qn(dir.path(), &["setup", "--print-config"]);
    assert!(printed.status.success());
    let text = stdout(&printed);
    assert!(text.contains("theme = \"kanagawa-wave\""));
    assert!(text.contains("# theme:"));
    assert!(!dir.path().join("config.toml").exists());

    let written = qn(dir.path(), &["setup"]);
    assert!(written.status.success());
    assert!(dir.path().join("config.toml").exists());
    assert!(stdout(&qn(dir.path(), &["setup"])).contains("already exists"));
}

#[test]
fn printed_config_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    let text = stdout(&qn(dir.path(), &["setup", "--print-config"]));
    let config: quicknotes::config::Config = toml::from_str(&text).unwrap();
    assert_eq!(config, quicknotes::config::Config::default());
}

#[test]
fn doctor_reports_and_succeeds_on_a_fresh_install() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());

    let output = qn(dir.path(), &["doctor"]);
    let text = stdout(&output);

    assert!(output.status.success(), "{text}");
    assert!(text.contains("quicknotes doctor"));
    assert!(text.contains("FTS5 index is consistent"));
}

#[test]
fn doctor_fails_with_a_broken_config() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "icons = 3").unwrap();

    let output = qn(dir.path(), &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("problem(s)"));
}

#[test]
fn export_writes_markdown_files() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());

    let output = qn(dir.path(), &["export", "out"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("exported 3 item(s)"));

    let mut names: Vec<String> = std::fs::read_dir(dir.path().join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 3);
    assert!(
        names
            .iter()
            .any(|name| name.starts_with("note-") && name.ends_with("-plan.md"))
    );

    let snippet = names
        .iter()
        .find(|name| name.starts_with("snippet-"))
        .unwrap();
    let text = std::fs::read_to_string(dir.path().join("out").join(snippet)).unwrap();
    assert!(text.starts_with("---\n"));
    assert!(text.contains("```rust\nfn main() {}\n```"));
}

#[test]
fn export_filters_and_streams_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());

    let output = qn(dir.path(), &["export", "-", "--tag", "work"]);
    let text = stdout(&output);
    assert!(text.contains("title: \"Plan\""));
    assert!(!text.contains("Hello"));

    let todos = qn(dir.path(), &["export", "-", "--kind", "todos"]);
    let text = stdout(&todos);
    assert!(text.contains("kind: todo"));
    assert!(!text.contains("kind: note"));
}

#[test]
fn piping_the_default_command_prints_a_summary() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());

    let text = stdout(&qn(dir.path(), &[]));
    assert!(text.contains("1 notes"));
    assert!(text.contains("1 snippets"));
}
