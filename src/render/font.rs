//! Font loading and cell metrics.
//!
//! Owns a `cosmic_text::FontSystem` populated with only the bundled
//! JetBrains Mono Nerd Font Regular face. The bundled-only approach
//! keeps renders deterministic across machines: no operator-system font
//! library, no fontconfig surprises, no missing-glyph fallback drift.

use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache,
    fontdb::{Database, Source},
};

/// JetBrains Mono Nerd Font Regular, embedded at compile time.
///
/// SIL OFL 1.1 licensed; see `LICENSE`.
const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../assets/JetBrainsMonoNerdFont-Regular.ttf");

/// The bundled font's family name as fontdb sees it.
pub const FONT_FAMILY: &str = "JetBrainsMono Nerd Font";

/// Build a fresh `FontSystem` carrying only the bundled font.
///
/// Skips operator-system font discovery so renders are reproducible
/// across hosts.
pub fn build_font_system() -> FontSystem {
    let mut db = Database::new();
    db.set_monospace_family(FONT_FAMILY);
    db.load_font_source(Source::Binary(Arc::new(JETBRAINS_MONO_REGULAR)));
    FontSystem::new_with_locale_and_db(String::from("en-US"), db)
}

/// Measured cell metrics: monospace advance width plus full line height.
#[derive(Debug, Clone, Copy)]
pub struct CellMetrics {
    pub width: u32,
    pub height: u32,
    pub baseline: f32,
}

/// Measure the cell box for the bundled font at `font_size` points.
///
/// Shapes the string `"M"` (the canonical monospace measuring glyph)
/// into a single-line buffer and reads back the line's height and the
/// glyph's advance width. The returned width is the monospace cell
/// width: every glyph in JetBrains Mono shapes to the same advance.
pub fn measure(font_system: &mut FontSystem, font_size: f32) -> CellMetrics {
    let metrics = Metrics::new(font_size, font_size * 1.25);
    let mut buffer = Buffer::new(font_system, metrics);
    let attrs = Attrs::new().family(Family::Name(FONT_FAMILY));
    buffer.set_text(font_system, "M", attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    let mut width = font_size.ceil() as u32;
    let mut height = (font_size * 1.25).ceil() as u32;
    let mut baseline = font_size;
    if let Some(run) = buffer.layout_runs().next() {
        height = run.line_height.ceil() as u32;
        baseline = run.line_y - run.line_top;
        if let Some(glyph) = run.glyphs.first() {
            width = glyph.w.ceil() as u32;
        }
    }
    CellMetrics {
        width: width.max(1),
        height: height.max(1),
        baseline,
    }
}

/// Lazily-built per-render scratch state.
///
/// Holds the swash glyph cache and a reusable single-cell buffer so the
/// per-cell draw routine does not re-allocate for every cell. The font
/// system is owned by the caller and threaded through every draw call.
pub struct GlyphScratch {
    pub swash: SwashCache,
    pub buffer: Buffer,
}

impl GlyphScratch {
    /// Allocate a scratch context tied to a metrics setting.
    pub fn new(font_system: &mut FontSystem, font_size: f32) -> Self {
        let metrics = Metrics::new(font_size, font_size * 1.25);
        let buffer = Buffer::new(font_system, metrics);
        Self {
            swash: SwashCache::new(),
            buffer,
        }
    }
}
