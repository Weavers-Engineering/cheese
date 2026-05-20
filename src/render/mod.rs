//! Grid-to-Pixmap rendering pipeline.
//!
//! Takes the `Grid` extracted by `vt::parse` and paints a `Pixmap` that
//! can be encoded as PNG. Layout is row-major: cell `(row, col)` lands
//! at pixel `(padding + col * cell_w, padding + chrome_h + row * cell_h)`.
//! Chrome is a Phase 5 stub here; Phase 3 always renders a plain canvas.

use anyhow::{Context, Result};
use tiny_skia::Pixmap;

use crate::{exec::Chrome, theme::Theme, vt::Grid};

pub mod cell;
pub mod cursor;
pub mod font;

/// Knobs for a single render call.
pub struct RenderOpts {
    pub theme: Theme,
    pub font_size: f32,
    pub padding: u32,
    pub chrome: Chrome,
    /// Phase 5 will use this. Held here so the option surface is stable.
    pub no_shadow: bool,
}

/// Render a parsed grid into a fresh `Pixmap`.
///
/// # Errors
///
/// Returns `Err` when the requested pixmap dimensions overflow
/// `tiny_skia`'s allocation budget (extremely unlikely at terminal
/// scales).
pub fn draw(grid: &Grid, opts: &RenderOpts) -> Result<Pixmap> {
    let mut font_system = font::build_font_system();
    let metrics = font::measure(&mut font_system, opts.font_size);
    let mut scratch = font::GlyphScratch::new(&mut font_system, opts.font_size);

    let inner_w = grid.cols as u32 * metrics.width;
    let inner_h = grid.used_rows as u32 * metrics.height;
    let chrome_h: u32 = match opts.chrome {
        Chrome::None => 0,
        Chrome::Mac => 0, // Phase 5 fills this in.
    };
    let w = inner_w + opts.padding * 2;
    let h = inner_h + opts.padding * 2 + chrome_h;

    let mut pixmap = Pixmap::new(w, h).context("allocating pixmap")?;
    pixmap.fill(Theme::parse_color(&opts.theme.background));

    {
        let mut ctx = cell::CellContext {
            pixmap: &mut pixmap,
            font_system: &mut font_system,
            scratch: &mut scratch,
            metrics,
            theme: &opts.theme,
        };
        for (i, c) in grid.cells.iter().enumerate() {
            let row = i / grid.cols;
            if row >= grid.used_rows {
                break;
            }
            let col = i % grid.cols;
            let x = opts.padding + col as u32 * metrics.width;
            let y = opts.padding + chrome_h + row as u32 * metrics.height;
            cell::draw(&mut ctx, *c, x, y);
        }
    }

    if let Some((row, col)) = grid.cursor
        && row < grid.used_rows
    {
        let x = opts.padding + col as u32 * metrics.width;
        let y = opts.padding + chrome_h + row as u32 * metrics.height;
        cursor::draw(
            &mut pixmap,
            x,
            y,
            metrics.width,
            metrics.height,
            &opts.theme,
        );
    }

    Ok(pixmap)
}
