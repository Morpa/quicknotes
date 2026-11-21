use quicknotes::config::{ColorMode, Config, ThemeName};

#[test]
fn default_config_roundtrips_through_toml() {
    let text = Config::default().to_toml().unwrap();
    let parsed: Config = toml::from_str(&text).unwrap();

    assert_eq!(parsed, Config::default());
    assert!(text.contains("theme = \"kanagawa-wave\""));
    assert!(text.contains("color_mode = \"truecolor\""));
    assert!(text.contains("# quicknotes configuration"));
}

#[test]
fn ensure_file_creates_the_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/config.toml");

    assert!(Config::ensure_file(&path).unwrap());
    assert!(!Config::ensure_file(&path).unwrap());

    let loaded = Config::load_from(&path).unwrap();
    assert_eq!(loaded, Config::default());
}

#[test]
fn invalid_values_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "theme = \"solarized\"\n").unwrap();

    assert!(Config::load_from(&path).is_err());
}

#[test]
fn unknown_keys_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        "theme = \"kanagawa-dragon\"\ncolor_mode = \"256\"\nfuture_option = true\n",
    )
    .unwrap();

    let config = Config::load_from(&path).unwrap();
    assert_eq!(config.theme, ThemeName::Dragon);
    assert_eq!(config.color_mode, ColorMode::Ansi256);
}

#[test]
fn a_partial_file_keeps_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "accent = \"carpYellow\"\n").unwrap();

    let config = Config::load_from(&path).unwrap();
    assert_eq!(config.theme, ThemeName::Wave);
    assert_eq!(config.accent.as_str(), "carpYellow");
    assert_eq!(config.color_mode, ColorMode::Truecolor);
}
