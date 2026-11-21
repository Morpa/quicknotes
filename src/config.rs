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
# theme:       kanagawa-wave | kanagawa-dragon | kanagawa-lotus
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
}

impl ThemeName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wave => "kanagawa-wave",
            Self::Dragon => "kanagawa-dragon",
            Self::Lotus => "kanagawa-lotus",
        }
    }

    /// Cycles Wave → Dragon → Lotus, useful for a future key binding.
    pub const fn next(self) -> Self {
        match self {
            Self::Wave => Self::Dragon,
            Self::Dragon => Self::Lotus,
            Self::Lotus => Self::Wave,
        }
    }
}

impl fmt::Display for ThemeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Wave => "Kanagawa Wave",
            Self::Dragon => "Kanagawa Dragon",
            Self::Lotus => "Kanagawa Lotus",
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
            other => Err(format!(
                "unknown theme `{other}` (expected kanagawa-wave, kanagawa-dragon or kanagawa-lotus)"
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

    pub fn to_toml(&self) -> Result<String> {
        let body = toml::to_string_pretty(self).context("serializing configuration")?;
        Ok(format!("{CONFIG_HEADER}\n{body}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
