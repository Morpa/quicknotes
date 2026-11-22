//! User configuration, loaded from `config.toml`.
//!
//! The configuration is intentionally small: appearance plus an optional
//! database location. A missing or invalid file never prevents the app from
//! starting — defaults are used and the problem is logged.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

const CONFIG_HEADER: &str = "\
# quicknotes configuration
#
# theme:       kanagawa-wave | kanagawa-dragon | kanagawa-lotus | auto
#              (auto asks the terminal for its background: dark -> wave, light -> lotus)
# background:  terminal (blend with your terminal) | sumi (solid Kanagawa ink)
# accent:      crystalBlue | springBlue | carpYellow | sakuraPink | waveAqua1 | springGreen | surimiOrange
# icons:       nerd | ascii
# color_mode:  truecolor | 256
# db_path:     optional absolute path (or ~/...) for the SQLite database
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeName {
    #[default]
    #[serde(rename = "kanagawa-wave")]
    Wave,
    #[serde(rename = "kanagawa-dragon")]
    Dragon,
    #[serde(rename = "kanagawa-lotus")]
    Lotus,
    /// Resolved at startup from the terminal background (OSC 11); Wave when
    /// the terminal does not answer.
    #[serde(rename = "auto")]
    Auto,
}

impl ThemeName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wave => "kanagawa-wave",
            Self::Dragon => "kanagawa-dragon",
            Self::Lotus => "kanagawa-lotus",
            Self::Auto => "auto",
        }
    }

    /// Cycles Wave → Dragon → Lotus. `Auto` joins the cycle at Wave.
    pub const fn next(self) -> Self {
        match self {
            Self::Auto | Self::Lotus => Self::Wave,
            Self::Wave => Self::Dragon,
            Self::Dragon => Self::Lotus,
        }
    }

    /// The concrete variant; `Auto` falls back to Wave.
    pub const fn resolved(self) -> Self {
        match self {
            Self::Auto => Self::Wave,
            other => other,
        }
    }
}

impl fmt::Display for ThemeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Wave => "Kanagawa Wave",
            Self::Dragon => "Kanagawa Dragon",
            Self::Lotus => "Kanagawa Lotus",
            Self::Auto => "Kanagawa (auto)",
        };
        formatter.write_str(label)
    }
}

impl FromStr for ThemeName {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "kanagawa-wave" => Ok(Self::Wave),
            "kanagawa-dragon" => Ok(Self::Dragon),
            "kanagawa-lotus" => Ok(Self::Lotus),
            "auto" => Ok(Self::Auto),
            other => Err(format!(
                "unknown theme `{other}` (expected kanagawa-wave, kanagawa-dragon, kanagawa-lotus or auto)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Background {
    /// Use the terminal background (`Color::Reset`).
    #[default]
    #[serde(rename = "terminal")]
    Terminal,
    /// Paint the Kanagawa ink background.
    #[serde(rename = "sumi")]
    Sumi,
}

impl Background {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Sumi => "sumi",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Icons {
    #[default]
    #[serde(rename = "nerd")]
    Nerd,
    #[serde(rename = "ascii")]
    Ascii,
}

impl Icons {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nerd => "nerd",
            Self::Ascii => "ascii",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColorMode {
    #[default]
    #[serde(rename = "truecolor")]
    Truecolor,
    /// Quantize every color to the xterm-256 palette.
    #[serde(rename = "256")]
    Ansi256,
}

impl ColorMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Truecolor => "truecolor",
            Self::Ansi256 => "256",
        }
    }
}

/// Accent color, resolved against the active Kanagawa variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    CrystalBlue,
    SpringBlue,
    CarpYellow,
    SakuraPink,
    WaveAqua1,
    SpringGreen,
    SurimiOrange,
}

impl Accent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CrystalBlue => "crystalBlue",
            Self::SpringBlue => "springBlue",
            Self::CarpYellow => "carpYellow",
            Self::SakuraPink => "sakuraPink",
            Self::WaveAqua1 => "waveAqua1",
            Self::SpringGreen => "springGreen",
            Self::SurimiOrange => "surimiOrange",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeName,
    pub background: Background,
    pub accent: Accent,
    pub icons: Icons,
    pub color_mode: ColorMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<PathBuf>,
}

impl Config {
    /// Loads the configuration from the default location, falling back to
    /// defaults (and logging a warning) when the file is missing or invalid.
    pub fn load() -> Self {
        let path = match paths::config_file() {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(%error, "could not resolve the configuration path; using defaults");
                return Self::default();
            }
        };

        if !path.exists() {
            return Self::default();
        }

        match Self::load_from(&path) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %format!("{error:#}"),
                    "invalid configuration; using defaults"
                );
                Self::default()
            }
        }
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading configuration {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing configuration {}", path.display()))
    }

    /// Writes a commented default configuration if the file does not exist.
    /// Returns `true` when a file was created.
    pub fn ensure_file(path: &Path) -> Result<bool> {
        if path.exists() {
            return Ok(false);
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(path, Self::default().to_toml()?)
            .with_context(|| format!("writing configuration {}", path.display()))?;
        Ok(true)
    }

    /// Writes `theme = "..."` into the file at `path`, leaving every other
    /// line and comment alone. Creates the file when it does not exist.
    pub fn save_theme(path: &Path, theme: ThemeName) -> Result<()> {
        if !path.exists() {
            let config = Self {
                theme,
                ..Self::default()
            };
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            return std::fs::write(path, config.to_toml()?)
                .with_context(|| format!("writing configuration {}", path.display()));
        }

        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading configuration {}", path.display()))?;
        let updated = replace_theme(&text, theme);
        if updated == text {
            return Ok(());
        }

        // Write next to the target and rename, so a crash never leaves a
        // half-written configuration behind.
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, updated)
            .with_context(|| format!("writing {}", temporary.display()))?;
        std::fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))
    }

    /// Replaces `theme = "auto"` with Lotus (light terminal) or Wave (dark).
    /// Other themes are left alone.
    pub fn resolve_auto(mut self, dark_terminal: Option<bool>) -> Self {
        if self.theme == ThemeName::Auto {
            self.theme = match dark_terminal {
                Some(false) => ThemeName::Lotus,
                _ => ThemeName::Wave,
            };
        }
        self
    }

    pub fn to_toml(&self) -> Result<String> {
        let body = toml::to_string_pretty(self).context("serializing configuration")?;
        Ok(format!("{CONFIG_HEADER}\n{body}"))
    }
}

/// Swaps the value of the top-level `theme` key, keeping an inline comment.
/// Appends the key when the text has none.
fn replace_theme(text: &str, theme: ThemeName) -> String {
    let mut found = false;
    let mut in_table = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('[') {
                in_table = true;
            }

            let is_theme_key = !in_table
                && !found
                && trimmed
                    .strip_prefix("theme")
                    .is_some_and(|rest| rest.trim_start().starts_with('='));
            if !is_theme_key {
                return line.to_string();
            }

            found = true;
            let comment = line
                .split_once('#')
                .map(|(_, comment)| format!("  #{comment}"))
                .unwrap_or_default();
            format!("theme = \"{}\"{comment}", theme.as_str())
        })
        .collect();

    if !found {
        // A top-level key must come before the first table.
        let position = lines
            .iter()
            .position(|line| line.trim_start().starts_with('['))
            .unwrap_or(lines.len());
        lines.insert(position, format!("theme = \"{}\"", theme.as_str()));
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_theme_and_keeps_comments() {
        let text = "# my notes\ntheme       = \"kanagawa-wave\"   # pick one\nicons = \"nerd\"\n";
        let updated = replace_theme(text, ThemeName::Lotus);

        assert_eq!(
            updated,
            "# my notes\ntheme = \"kanagawa-lotus\"  # pick one\nicons = \"nerd\"\n"
        );
    }

    #[test]
    fn ignores_commented_theme_lines_and_tables() {
        let text = "# theme = \"kanagawa-wave\"\nicons = \"nerd\"\n[other]\ntheme = \"x\"\n";
        let updated = replace_theme(text, ThemeName::Dragon);

        assert!(updated.contains("# theme = \"kanagawa-wave\""));
        assert!(updated.contains("[other]\ntheme = \"x\""));
        assert!(updated.starts_with(
            "# theme = \"kanagawa-wave\"\nicons = \"nerd\"\ntheme = \"kanagawa-dragon\"\n[other]"
        ));
    }

    #[test]
    fn saves_into_existing_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.toml");

        Config::save_theme(&path, ThemeName::Dragon).unwrap();
        assert_eq!(Config::load_from(&path).unwrap().theme, ThemeName::Dragon);

        Config::save_theme(&path, ThemeName::Lotus).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# theme:"), "comments survive");
        assert_eq!(Config::load_from(&path).unwrap().theme, ThemeName::Lotus);
        assert!(!path.with_extension("toml.tmp").exists());
    }

    #[test]
    fn default_config_roundtrips() {
        let text = Config::default().to_toml().expect("serialize");
        let parsed: Config = toml::from_str(&text).expect("parse");
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn theme_names_roundtrip() {
        for name in [ThemeName::Wave, ThemeName::Dragon, ThemeName::Lotus] {
            assert_eq!(name.as_str().parse::<ThemeName>(), Ok(name));
        }
    }

    #[test]
    fn auto_resolves_from_the_terminal_background() {
        let auto = Config {
            theme: ThemeName::Auto,
            ..Config::default()
        };
        assert_eq!(
            auto.clone().resolve_auto(Some(false)).theme,
            ThemeName::Lotus
        );
        assert_eq!(auto.clone().resolve_auto(Some(true)).theme, ThemeName::Wave);
        assert_eq!(auto.resolve_auto(None).theme, ThemeName::Wave);

        let fixed = Config {
            theme: ThemeName::Dragon,
            ..Config::default()
        };
        assert_eq!(fixed.resolve_auto(Some(false)).theme, ThemeName::Dragon);
    }

    #[test]
    fn auto_parses_from_toml() {
        let config: Config = toml::from_str("theme = \"auto\"").unwrap();
        assert_eq!(config.theme, ThemeName::Auto);
        assert_eq!(ThemeName::Auto.next(), ThemeName::Wave);
    }

    #[test]
    fn cycles_through_variants() {
        assert_eq!(ThemeName::Wave.next(), ThemeName::Dragon);
        assert_eq!(ThemeName::Dragon.next(), ThemeName::Lotus);
        assert_eq!(ThemeName::Lotus.next(), ThemeName::Wave);
    }

    #[test]
    fn color_mode_uses_the_256_alias() {
        let text = Config {
            color_mode: ColorMode::Ansi256,
            ..Config::default()
        }
        .to_toml()
        .expect("serialize");
        assert!(text.contains("color_mode = \"256\""));
        assert_eq!(ColorMode::Ansi256.as_str(), "256");
    }
}
