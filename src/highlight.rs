//! Syntax highlighting for snippets.
//!
//! The syntect theme is built from the active Kanagawa palette, so Wave,
//! Dragon and Lotus all highlight with their own colors. Only foreground
//! colors are emitted: the terminal (or sumi) background always shows through.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, StyleModifier, Theme as SynTheme, ThemeItem,
    ThemeSettings,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::config::{ColorMode, ThemeName};
use crate::ui::theme::{Rgb, Theme, resolve, syntax_palette};

fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn color(rgb: Rgb) -> SynColor {
    SynColor {
        r: rgb.0,
        g: rgb.1,
        b: rgb.2,
        a: 0xff,
    }
}

fn rule(scopes: &str, foreground: Rgb, font_style: Option<FontStyle>) -> ThemeItem {
    ThemeItem {
        scope: ScopeSelectors::from_str(scopes).expect("static scope selector is valid"),
        style: StyleModifier {
            foreground: Some(color(foreground)),
            background: None,
            font_style,
        },
    }
}

/// Builds the syntect theme (the in-memory equivalent of a `.tmTheme`) for a
/// Kanagawa variant.
pub fn syntect_theme(name: ThemeName) -> SynTheme {
    let palette = syntax_palette(name);

    SynTheme {
        name: Some(format!("kanagawa-{name:?}").to_lowercase()),
        author: Some("quicknotes".to_string()),
        settings: ThemeSettings {
            foreground: Some(color(palette.text)),
            ..ThemeSettings::default()
        },
        scopes: vec![
            rule(
                "comment, punctuation.definition.comment",
                palette.comment,
                Some(FontStyle::ITALIC),
            ),
            rule(
                "string, punctuation.definition.string",
                palette.string,
                None,
            ),
            rule("constant.character.escape", palette.escape, None),
            rule("constant.numeric", palette.number, None),
            rule(
                "constant, support.constant, variable.language",
                palette.constant,
                None,
            ),
            rule(
                "keyword, storage, storage.type, storage.modifier",
                palette.keyword,
                None,
            ),
            rule("keyword.operator", palette.operator, None),
            rule(
                "entity.name.function, support.function, meta.function-call variable.function",
                palette.function,
                None,
            ),
            rule(
                "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
                 entity.other.inherited-class, support.type, support.class",
                palette.type_name,
                None,
            ),
            rule("entity.name.tag, markup.deleted", palette.tag, None),
            rule("entity.other.attribute-name", palette.constant, None),
            rule("variable.parameter", palette.text, Some(FontStyle::ITALIC)),
            rule("punctuation", palette.punctuation, None),
        ],
    }
}

/// Maps the language names people type to a syntax in syntect's default pack.
fn find_syntax(language: Option<&str>) -> &'static SyntaxReference {
    let set = syntaxes();
    let Some(language) = language.map(str::trim).filter(|value| !value.is_empty()) else {
        return set.find_syntax_plain_text();
    };

    let lowered = language.to_lowercase();
    let token = match lowered.as_str() {
        "typescript" | "ts" | "tsx" | "jsx" | "javascript" | "node" => "js",
        "shell" | "zsh" | "bash" | "fish" => "sh",
        "python" => "py",
        "yml" => "yaml",
        "golang" => "go",
        "c++" | "cxx" => "cpp",
        "c#" | "csharp" => "cs",
        "markdown" => "md",
        other => other,
    };

    set.find_syntax_by_token(token)
        .or_else(|| set.find_syntax_by_token(&lowered))
        .unwrap_or_else(|| set.find_syntax_plain_text())
}

/// Style for one highlighted token.
fn token_style(style: syntect::highlighting::Style, mode: ColorMode) -> Style {
    let rgb = Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let mut output = Style::default().fg(resolve(rgb, mode));
    if style.font_style.contains(FontStyle::ITALIC) {
        output = output.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::BOLD) {
        output = output.add_modifier(Modifier::BOLD);
    }
    output
}

fn highlight_uncached(body: &str, language: Option<&str>, theme: &Theme) -> Text<'static> {
    let syntax = find_syntax(language);
    let syntect = syntect_theme(theme.name);
    let mut highlighter = HighlightLines::new(syntax, &syntect);
    let set = syntaxes();

    let lines: Vec<Line<'static>> = LinesWithEndings::from(body)
        .map(|line| {
            let tokens = match highlighter.highlight_line(line, set) {
                Ok(tokens) => tokens,
                Err(_) => return Line::from(line.trim_end_matches(['\n', '\r']).to_string()),
            };

            let spans: Vec<Span<'static>> = tokens
                .into_iter()
                .filter_map(|(style, text)| {
                    let text = text.trim_end_matches(['\n', '\r']);
                    (!text.is_empty()).then(|| {
                        Span::styled(text.to_string(), token_style(style, theme.color_mode))
                    })
                })
                .collect();
            Line::from(spans)
        })
        .collect();

    Text::from(lines)
}

type CacheKey = (u64, Option<String>, ThemeName, ColorMode);

thread_local! {
    /// The preview and the editor re-render the same snippet on every tick;
    /// remembering the last result keeps idle redraws cheap.
    static LAST: RefCell<Option<(CacheKey, Text<'static>)>> = const { RefCell::new(None) };
}

/// Highlights `body` as `language`, falling back to plain text for unknown
/// languages. Always returns owned text.
pub fn highlight(body: &str, language: Option<&str>, theme: &Theme) -> Text<'static> {
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    let key: CacheKey = (
        hasher.finish(),
        language.map(str::to_lowercase),
        theme.name,
        theme.color_mode,
    );

    if let Some(hit) = LAST.with(|last| {
        last.borrow()
            .as_ref()
            .filter(|(cached, _)| *cached == key)
            .map(|(_, text)| text.clone())
    }) {
        return hit;
    }

    let text = highlight_uncached(body, language, theme);
    LAST.with(|last| *last.borrow_mut() = Some((key, text.clone())));
    text
}

/// Foreground color of the first span containing `needle`, for tests.
pub fn color_of(text: &Text<'_>, needle: &str) -> Option<Color> {
    text.lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .find(|span| span.content.contains(needle))
        .and_then(|span| span.style.fg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Accent, Background};

    fn theme(name: ThemeName) -> Theme {
        Theme::new(
            name,
            Background::Terminal,
            Accent::CrystalBlue,
            ColorMode::Truecolor,
        )
    }

    #[test]
    fn rust_keywords_use_the_violet_role() {
        let text = highlight("fn main() {}\n", Some("rust"), &theme(ThemeName::Wave));
        assert_eq!(
            color_of(&text, "fn"),
            Some(Color::Rgb(0x95, 0x7F, 0xB8)),
            "{text:?}"
        );
    }

    #[test]
    fn strings_are_green_and_comments_are_dim() {
        let text = highlight(
            "let a = \"hi\"; // note\n",
            Some("rust"),
            &theme(ThemeName::Wave),
        );
        assert_eq!(color_of(&text, "hi"), Some(Color::Rgb(0x98, 0xBB, 0x6C)));
        assert_eq!(color_of(&text, "note"), Some(Color::Rgb(0x72, 0x71, 0x69)));
    }

    #[test]
    fn variants_use_their_own_palette() {
        let wave = highlight("fn a() {}\n", Some("rust"), &theme(ThemeName::Wave));
        let lotus = highlight("fn a() {}\n", Some("rust"), &theme(ThemeName::Lotus));
        assert_ne!(color_of(&wave, "fn"), color_of(&lotus, "fn"));
    }

    #[test]
    fn aliases_and_unknown_languages_do_not_panic() {
        let theme = theme(ThemeName::Wave);
        for language in [Some("py"), Some("sh"), Some("ts"), Some("nope"), None] {
            let text = highlight("x = 1\n", language, &theme);
            assert_eq!(text.to_string(), "x = 1");
        }
    }

    #[test]
    fn highlighting_preserves_the_text() {
        let body = "fn main() {\n    println!(\"hi\");\n}\n\nlast line";
        let text = highlight(body, Some("rust"), &theme(ThemeName::Dragon));
        assert_eq!(text.to_string(), body.trim_end_matches('\n'));
    }

    #[test]
    fn ansi256_mode_quantizes_colors() {
        let theme = Theme::new(
            ThemeName::Wave,
            Background::Terminal,
            Accent::CrystalBlue,
            ColorMode::Ansi256,
        );
        let text = highlight("fn a() {}\n", Some("rust"), &theme);
        assert!(matches!(color_of(&text, "fn"), Some(Color::Indexed(_))));
    }
}
