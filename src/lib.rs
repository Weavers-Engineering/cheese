//! Public library surface for `cheese`.
//!
//! Re-exports the modules that drive `cheese exec` so downstream
//! crates (and the binary) share one source of truth.

pub mod exec;
pub mod pipe;
pub mod pty;
pub mod render;
pub mod theme;
pub mod vt;

use anyhow::Result;

use crate::{
    exec::Chrome,
    render::{RenderOpts, draw},
    theme::Theme,
    vt::parse,
};

/// Deterministic render-from-bytes used by the golden test suite.
///
/// Skips the PTY: feeds `bytes` straight through libghostty-vt at the
/// given `cols` x `rows`, paints the resulting grid with `opts`, and
/// returns the encoded PNG. Same bytes plus same opts in, same PNG out,
/// on every host that ships the same bundled font.
///
/// # Errors
///
/// Returns `Err` when libghostty-vt rejects the bytes, when the pixmap
/// allocation overflows, or when PNG encoding fails.
pub fn render_canned_to_png(
    bytes: &[u8],
    cols: usize,
    rows: usize,
    opts: &RenderOpts,
) -> Result<Vec<u8>> {
    let grid = parse(bytes, cols, rows)?;
    let pixmap = draw(&grid, opts)?;
    let png = pixmap.encode_png()?;
    Ok(png)
}

/// Build the default `RenderOpts` used by the golden tests.
///
/// Tokyo Night Dark, font size 14, padding 24, no chrome, shadow off.
/// Mirrors the binary defaults so a golden divergence flags either a
/// real render regression or a deliberate default change.
pub fn test_render_opts() -> RenderOpts {
    RenderOpts {
        theme: Theme::tokyo_night_dark(),
        font_size: 14.0,
        padding: 24,
        chrome: Chrome::None,
        no_shadow: false,
    }
}
