//! `cheese exec` flow: spawn the command in a PTY, parse the captured
//! bytes through libghostty-vt, render the grid to a Pixmap, and write
//! the encoded PNG to disk.

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::{
    pty,
    render::{self, RenderOpts},
    theme::Theme,
    vt,
};

/// Arguments accepted by `cheese exec`.
///
/// All fields drive either PTY sizing (`cols`, `rows`), rendering
/// (`font_size`, `padding`, `chrome`, `theme`, `no_shadow`), or output
/// (`output`).
#[derive(Debug, Clone)]
pub struct ExecArgs {
    pub cmd: Vec<String>,
    pub cols: u16,
    pub rows: u16,
    pub output: PathBuf,
    pub font_size: f32,
    pub padding: u32,
    pub chrome: Chrome,
    pub theme: String,
    pub no_shadow: bool,
}

/// Optional window chrome around the cell region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Chrome {
    None,
    Mac,
}

/// Spawn the child, parse its output, render the grid, write the PNG.
///
/// # Errors
///
/// Returns `Err` when the PTY capture fails, the libghostty-vt parse
/// rejects the captured bytes, the theme name is unknown, the pixmap
/// allocation overflows, or the output path cannot be written.
pub fn run(args: ExecArgs) -> Result<()> {
    let captured = pty::run(&args.cmd, args.cols, args.rows)?;
    let grid = vt::parse(&captured, args.cols as usize, args.rows as usize)?;
    let theme = resolve_theme(&args.theme)?;
    let opts = RenderOpts {
        theme,
        font_size: args.font_size,
        padding: args.padding,
        chrome: args.chrome,
        no_shadow: args.no_shadow,
    };
    let pixmap = render::draw(&grid, &opts)?;
    pixmap
        .save_png(&args.output)
        .with_context(|| format!("writing png to {}", args.output.display()))?;
    eprintln!("wrote {}", args.output.display());
    Ok(())
}

fn resolve_theme(name: &str) -> Result<Theme> {
    match name {
        "tokyo-night-dark" => Ok(Theme::tokyo_night_dark()),
        other => Err(anyhow::anyhow!(
            "unknown theme {other:?}: v0.1 only ships tokyo-night-dark"
        )),
    }
}
