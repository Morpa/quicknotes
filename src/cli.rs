//! Command-line arguments. Hand-rolled: the surface is tiny and a parser
//! dependency would outweigh it.

use std::path::PathBuf;

use crate::db::Kind;

pub const HELP: &str = "\
quicknotes — minimal terminal notes, todos and code snippets

USAGE
    qn                          open the interface
    qn doctor                   check the installation and the terminal
    qn setup [--print-config]   create config.toml, or print the default one
    qn export [DIR | -] [--kind KIND] [--tag TAG]
                                write every item as Markdown with frontmatter
    qn --version | --help

EXPORT
    DIR            target directory, one .md per item (default: quicknotes-export)
    -              print everything to stdout instead
    --kind KIND    note | todo | snippet
    --tag TAG      only items with this tag
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportTarget {
    Dir(PathBuf),
    Stdout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportArgs {
    pub target: ExportTarget,
    pub kind: Option<Kind>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cli {
    Tui,
    Help,
    Version,
    Doctor,
    Setup { print_config: bool },
    Export(ExportArgs),
}

pub fn parse<I, S>(args: I) -> Result<Cli, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(first) = args.next() else {
        return Ok(Cli::Tui);
    };

    match first.as_str() {
        "-h" | "--help" | "help" => Ok(Cli::Help),
        "-V" | "--version" | "version" => Ok(Cli::Version),
        "doctor" => no_more(args, "doctor").map(|()| Cli::Doctor),
        "setup" => parse_setup(args),
        "export" => parse_export(args),
        other => Err(format!("unknown command `{other}`")),
    }
}

fn no_more(mut args: impl Iterator<Item = String>, command: &str) -> Result<(), String> {
    match args.next() {
        Some(extra) => Err(format!("`qn {command}` takes no argument `{extra}`")),
        None => Ok(()),
    }
}

fn parse_setup(args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut print_config = false;
    for arg in args {
        match arg.as_str() {
            "--print-config" => print_config = true,
            other => return Err(format!("unknown option `{other}` for `qn setup`")),
        }
    }
    Ok(Cli::Setup { print_config })
}

fn parse_kind(value: &str) -> Result<Kind, String> {
    value
        .trim_end_matches('s')
        .parse()
        .map_err(|_| format!("unknown kind `{value}` (expected note, todo or snippet)"))
}

fn parse_export(mut args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut target = None;
    let mut kind = None;
    let mut tag = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--kind" => {
                let value = args.next().ok_or("`--kind` needs a value")?;
                kind = Some(parse_kind(&value)?);
            }
            "--tag" => tag = Some(args.next().ok_or("`--tag` needs a value")?),
            "-" => set_target(&mut target, ExportTarget::Stdout)?,
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option `{flag}` for `qn export`"));
            }
            path => set_target(&mut target, ExportTarget::Dir(PathBuf::from(path)))?,
        }
    }

    Ok(Cli::Export(ExportArgs {
        target: target.unwrap_or_else(|| ExportTarget::Dir(PathBuf::from("quicknotes-export"))),
        kind,
        tag,
    }))
}

fn set_target(slot: &mut Option<ExportTarget>, value: ExportTarget) -> Result<(), String> {
    if slot.is_some() {
        return Err("`qn export` takes a single target".to_string());
    }
    *slot = Some(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(args: &[&str]) -> Result<Cli, String> {
        parse(args.iter().copied())
    }

    #[test]
    fn no_arguments_opens_the_interface() {
        assert_eq!(parse_all(&[]), Ok(Cli::Tui));
    }

    #[test]
    fn simple_commands() {
        assert_eq!(parse_all(&["doctor"]), Ok(Cli::Doctor));
        assert_eq!(parse_all(&["--version"]), Ok(Cli::Version));
        assert_eq!(parse_all(&["-h"]), Ok(Cli::Help));
        assert!(parse_all(&["doctor", "extra"]).is_err());
        assert!(parse_all(&["nope"]).is_err());
    }

    #[test]
    fn setup_flags() {
        assert_eq!(
            parse_all(&["setup"]),
            Ok(Cli::Setup {
                print_config: false
            })
        );
        assert_eq!(
            parse_all(&["setup", "--print-config"]),
            Ok(Cli::Setup { print_config: true })
        );
        assert!(parse_all(&["setup", "--bogus"]).is_err());
    }

    #[test]
    fn export_defaults_and_options() {
        assert_eq!(
            parse_all(&["export"]),
            Ok(Cli::Export(ExportArgs {
                target: ExportTarget::Dir("quicknotes-export".into()),
                kind: None,
                tag: None,
            }))
        );
        assert_eq!(
            parse_all(&["export", "out", "--kind", "snippets", "--tag", "rust"]),
            Ok(Cli::Export(ExportArgs {
                target: ExportTarget::Dir("out".into()),
                kind: Some(Kind::Snippet),
                tag: Some("rust".into()),
            }))
        );
        assert_eq!(
            parse_all(&["export", "-"]),
            Ok(Cli::Export(ExportArgs {
                target: ExportTarget::Stdout,
                kind: None,
                tag: None,
            }))
        );
    }

    #[test]
    fn export_rejects_bad_input() {
        assert!(parse_all(&["export", "--kind"]).is_err());
        assert!(parse_all(&["export", "--kind", "memo"]).is_err());
        assert!(parse_all(&["export", "a", "b"]).is_err());
        assert!(parse_all(&["export", "--format", "x"]).is_err());
    }
}
