//! Theme definitions and palette resolution.
//!
//! Two themes ship today: Tokyo Night Dark (default) and Ayu Dark. The
//! schema is JSON (background, foreground, cursor, selection, 16-color
//! palette); each theme ships embedded in the binary via `include_str!`.
//! v0.2 adds operator-supplied themes via `--theme-file`.

use serde::Deserialize;

const TOKYO_NIGHT_DARK_JSON: &str = include_str!("../assets/themes/tokyo-night-dark.json");
const AYU_DARK_JSON: &str = include_str!("../assets/themes/ayu-dark.json");

/// A complete terminal color scheme: chrome colors plus a 16-slot palette.
#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    pub name: String,
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    pub palette: Palette,
}

/// 16-color ANSI palette (8 normal + 8 bright).
///
/// Fields are stored as hex strings so the JSON theme files round-trip
/// cleanly. Use `Theme::parse_color` to lift a field into a
/// `tiny_skia::Color`.
#[derive(Debug, Clone, Deserialize)]
pub struct Palette {
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

impl Theme {
    /// The bundled Tokyo Night Dark theme.
    ///
    /// # Panics
    ///
    /// Panics if the embedded JSON asset fails to parse. The asset ships
    /// inside the binary; a parse failure means a broken build, not a
    /// runtime error a caller can recover from.
    pub fn tokyo_night_dark() -> Self {
        serde_json::from_str(TOKYO_NIGHT_DARK_JSON).expect("bundled theme parses")
    }

    /// The bundled Ayu Dark theme.
    ///
    /// # Panics
    ///
    /// Panics if the embedded JSON asset fails to parse. Build-time concern,
    /// not a runtime one.
    pub fn ayu_dark() -> Self {
        serde_json::from_str(AYU_DARK_JSON).expect("bundled theme parses")
    }

    /// Parse a `#rrggbb` hex string into a `tiny_skia::Color`.
    ///
    /// Returns opaque black on malformed input. Theme JSONs are
    /// hand-written; a typo there is a config bug worth surfacing
    /// visually rather than panicking.
    pub fn parse_color(s: &str) -> tiny_skia::Color {
        let s = s.trim_start_matches('#');
        if s.len() != 6 {
            return tiny_skia::Color::from_rgba8(0, 0, 0, 255);
        }
        let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
        tiny_skia::Color::from_rgba8(r, g, b, 255)
    }
}
