//! Kanagawa palettes mapped onto semantic UI roles.
//!
//! Colors are defined once here as RGB and converted to `ratatui` colors at
//! runtime, so truecolor terminals get the exact palette and 256-color
//! terminals get the nearest xterm entry.

use ratatui::style::{Color, Modifier, Style};

use crate::config::{Accent, Background, ColorMode, Config, ThemeName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

const fn hex(value: u32) -> Rgb {
    Rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

/// Raw colors for one Kanagawa variant, using role names rather than
/// variant-specific names (Wave's `crystalBlue` and Dragon's `dragonBlue2`
/// both become `blue`).
struct RawPalette {
    bg: Rgb,
    bg_dark: Rgb,
    surface: Rgb,
    fg: Rgb,
    fg_old: Rgb,
    dim: Rgb,
    faint: Rgb,
    border: Rgb,
    blue: Rgb,
    blue2: Rgb,
    yellow: Rgb,
    pink: Rgb,
    aqua: Rgb,
    green: Rgb,
    orange: Rgb,
    red: Rgb,
    red2: Rgb,
    violet: Rgb,
    sel_bg: Rgb,
    sel_fg: Rgb,
}

const WAVE: RawPalette = RawPalette {
    bg: hex(0x1F1F28),
    bg_dark: hex(0x16161D),
    surface: hex(0x2A2A37),
    fg: hex(0xDCD7BA),
    fg_old: hex(0xC8C093),
    dim: hex(0x727169),
    faint: hex(0x54546D),
    border: hex(0x363646),
    blue: hex(0x7E9CD8),
    blue2: hex(0x7FB4CA),
    yellow: hex(0xE6C384),
    pink: hex(0xD27E99),
    aqua: hex(0x6A9589),
    green: hex(0x98BB6C),
    orange: hex(0xFFA066),
    red: hex(0xE46876),
    red2: hex(0xFF5D62),
    violet: hex(0x957FB8),
    sel_bg: hex(0x223249),
    sel_fg: hex(0xDCD7BA),
};

const DRAGON: RawPalette = RawPalette {
    bg: hex(0x181616),
    bg_dark: hex(0x12120F),
    surface: hex(0x282727),
    fg: hex(0xC5C9C5),
    fg_old: hex(0xA6A69C),
    dim: hex(0x7A8382),
    faint: hex(0x625E5A),
    border: hex(0x393836),
    blue: hex(0x8BA4B0),
    blue2: hex(0x949FB5),
    yellow: hex(0xC4B28A),
    pink: hex(0xA292A3),
    aqua: hex(0x8EA4A2),
    green: hex(0x87A987),
    orange: hex(0xB6927B),
    red: hex(0xC4746E),
    red2: hex(0xC4746E),
    violet: hex(0x8992A7),
    sel_bg: hex(0x393836),
    sel_fg: hex(0xC5C9C5),
};

const LOTUS: RawPalette = RawPalette {
    bg: hex(0xF2ECBC),
    bg_dark: hex(0xE5DDB0),
    surface: hex(0xDCD5AC),
    fg: hex(0x545464),
    fg_old: hex(0x43436C),
    dim: hex(0x8A8980),
    faint: hex(0xA09CAC),
    border: hex(0xD5CEA3),
    blue: hex(0x4D699B),
    blue2: hex(0x6693BF),
    yellow: hex(0x836F4A),
    pink: hex(0xB35B79),
    aqua: hex(0x597B75),
    green: hex(0x6F894E),
    orange: hex(0xCC6D00),
    red: hex(0xC84053),
    red2: hex(0xD7474B),
    violet: hex(0x766B90),
    sel_bg: hex(0xC7D7E0),
    sel_fg: hex(0x43436C),
};

fn palette(name: ThemeName) -> &'static RawPalette {
    match name {
        ThemeName::Wave => &WAVE,
        ThemeName::Dragon => &DRAGON,
        ThemeName::Lotus => &LOTUS,
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: ThemeName,
    pub background: Background,
    pub color_mode: ColorMode,
    pub bg: Color,
    pub bg_dark: Color,
    pub surface: Color,
    pub fg: Color,
    pub fg_old: Color,
    pub dim: Color,
    pub faint: Color,
    pub border: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    /// Brighter red for overdue emphasis.
    pub danger_bright: Color,
    pub tag: Color,
    pub sel_bg: Color,
    pub sel_fg: Color,
    pub violet: Color,
}

impl Theme {
    pub fn from_config(config: &Config) -> Self {
        Self::new(
            config.theme,
            config.background,
            config.accent,
            config.color_mode,
        )
    }

    pub fn new(
        name: ThemeName,
        background: Background,
        accent: Accent,
        color_mode: ColorMode,
    ) -> Self {
        let raw = palette(name);
        let convert = |rgb: Rgb| resolve(rgb, color_mode);

        let accent = match accent {
            Accent::CrystalBlue => raw.blue,
            Accent::SpringBlue => raw.blue2,
            Accent::CarpYellow => raw.yellow,
            Accent::SakuraPink => raw.pink,
            Accent::WaveAqua1 => raw.aqua,
            Accent::SpringGreen => raw.green,
            Accent::SurimiOrange => raw.orange,
        };

        Self {
            name,
            background,
            color_mode,
            bg: match background {
                Background::Terminal => Color::Reset,
                Background::Sumi => convert(raw.bg),
            },
            bg_dark: convert(raw.bg_dark),
            surface: convert(raw.surface),
            fg: convert(raw.fg),
            fg_old: convert(raw.fg_old),
            dim: convert(raw.dim),
            faint: convert(raw.faint),
            border: convert(raw.border),
            accent: convert(accent),
            accent_alt: convert(raw.yellow),
            success: convert(raw.green),
            warning: convert(raw.orange),
            danger: convert(raw.red),
            danger_bright: convert(raw.red2),
            tag: convert(raw.aqua),
            sel_bg: convert(raw.sel_bg),
            sel_fg: convert(raw.sel_fg),
            violet: convert(raw.violet),
        }
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.fg)
    }

    pub fn title(&self) -> Style {
        Style::default().fg(self.fg).add_modifier(Modifier::BOLD)
    }

    pub fn label(&self) -> Style {
        Style::default().fg(self.dim).add_modifier(Modifier::BOLD)
    }

    pub fn dimmed(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn faint(&self) -> Style {
        Style::default().fg(self.faint)
    }

    pub fn selected(&self) -> Style {
        Style::default().fg(self.sel_fg).bg(self.sel_bg)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn accent_alt(&self) -> Style {
        Style::default().fg(self.accent_alt)
    }

    pub fn tag(&self) -> Style {
        Style::default().fg(self.tag)
    }

    pub fn violet(&self) -> Style {
        Style::default().fg(self.violet)
    }

    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn danger(&self) -> Style {
        Style::default().fg(self.danger)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Completed todos: dimmed and struck through.
    pub fn done(&self) -> Style {
        Style::default()
            .fg(self.dim)
            .add_modifier(Modifier::CROSSED_OUT)
    }
}

pub fn resolve(rgb: Rgb, mode: ColorMode) -> Color {
    match mode {
        ColorMode::Truecolor => Color::Rgb(rgb.0, rgb.1, rgb.2),
        ColorMode::Ansi256 => Color::Indexed(nearest_ansi256(rgb)),
    }
}

/// Nearest xterm-256 entry, restricted to the 6×6×6 cube and the grayscale
/// ramp (16..=255) so terminal-dependent ANSI colors are never used.
pub fn nearest_ansi256(color: Rgb) -> u8 {
    let mut best = 16u8;
    let mut best_distance = u32::MAX;

    for index in 16u8..=255 {
        let distance = distance(color, ansi256_rgb(index));
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
        if distance == 0 {
            break;
        }
    }

    best
}

fn ansi256_rgb(index: u8) -> Rgb {
    match index {
        16..=231 => {
            let offset = u16::from(index) - 16;
            let r = (offset / 36) as u8;
            let g = ((offset % 36) / 6) as u8;
            let b = (offset % 6) as u8;
            Rgb(level(r), level(g), level(b))
        }
        232..=255 => {
            let value = 8 + 10 * (index - 232);
            Rgb(value, value, value)
        }
        _ => Rgb(0, 0, 0),
    }
}

fn level(value: u8) -> u8 {
    match value {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        _ => 255,
    }
}

fn distance(a: Rgb, b: Rgb) -> u32 {
    let dr = u32::from(a.0).abs_diff(u32::from(b.0));
    let dg = u32::from(a.1).abs_diff(u32::from(b.1));
    let db = u32::from(a.2).abs_diff(u32::from(b.2));
    dr * dr + dg * dg + db * db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_defaults_match_the_palette() {
        let theme = Theme::new(
            ThemeName::Wave,
            Background::Terminal,
            Accent::CrystalBlue,
            ColorMode::Truecolor,
        );
        assert_eq!(theme.bg, Color::Reset);
        assert_eq!(theme.fg, Color::Rgb(0xDC, 0xD7, 0xBA));
        assert_eq!(theme.accent, Color::Rgb(0x7E, 0x9C, 0xD8));
        assert_eq!(theme.sel_bg, Color::Rgb(0x22, 0x32, 0x49));
        assert_eq!(theme.danger, Color::Rgb(0xE4, 0x68, 0x76));
    }

    #[test]
    fn sumi_background_paints_the_ink_color() {
        let theme = Theme::new(
            ThemeName::Wave,
            Background::Sumi,
            Accent::CarpYellow,
            ColorMode::Truecolor,
        );
        assert_eq!(theme.bg, Color::Rgb(0x1F, 0x1F, 0x28));
        assert_eq!(theme.accent, Color::Rgb(0xE6, 0xC3, 0x84));
    }

    #[test]
    fn lotus_is_light() {
        let theme = Theme::new(
            ThemeName::Lotus,
            Background::Sumi,
            Accent::CrystalBlue,
            ColorMode::Truecolor,
        );
        assert_eq!(theme.bg, Color::Rgb(0xF2, 0xEC, 0xBC));
        assert_eq!(theme.fg, Color::Rgb(0x54, 0x54, 0x64));
    }

    #[test]
    fn ansi256_mode_quantizes_everything_except_terminal_background() {
        let theme = Theme::new(
            ThemeName::Dragon,
            Background::Sumi,
            Accent::SpringBlue,
            ColorMode::Ansi256,
        );
        for color in [
            theme.bg,
            theme.fg,
            theme.accent,
            theme.sel_bg,
            theme.border,
            theme.danger,
        ] {
            assert!(matches!(color, Color::Indexed(_)), "{color:?}");
        }

        let terminal = Theme::new(
            ThemeName::Dragon,
            Background::Terminal,
            Accent::SpringBlue,
            ColorMode::Ansi256,
        );
        assert_eq!(terminal.bg, Color::Reset);
    }

    #[test]
    fn quantizer_finds_exact_entries() {
        assert_eq!(nearest_ansi256(Rgb(0, 0, 0)), 16);
        assert_eq!(nearest_ansi256(Rgb(255, 255, 255)), 231);
        assert_eq!(nearest_ansi256(Rgb(128, 128, 128)), 244);
        assert_eq!(nearest_ansi256(Rgb(255, 0, 0)), 196);
    }

    #[test]
    fn done_style_is_crossed_out() {
        let theme = Theme::new(
            ThemeName::Wave,
            Background::Terminal,
            Accent::CrystalBlue,
            ColorMode::Truecolor,
        );
        assert!(theme.done().add_modifier.contains(Modifier::CROSSED_OUT));
    }
}
