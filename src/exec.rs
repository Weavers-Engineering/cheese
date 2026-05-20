//! `cheese exec` flow: spawn the command in a PTY, parse the captured
//! bytes through libghostty-vt, then (in later phases) hand the grid to
//! the renderer.

use anyhow::Result;
use std::path::PathBuf;

use crate::{pty, vt};

/// Arguments accepted by `cheese exec`.
///
/// Phase 2 only consumes `cmd`, `cols`, `rows`. The rendering knobs
/// (`output`, `font_size`, `padding`, `chrome`, `theme`, `no_shadow`)
/// are reserved here so the CLI surface is stable before the renderer
/// lands in Phase 3.
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

/// Spawn the child, parse its output, and (later) render to PNG.
///
/// # Errors
///
/// Returns `Err` when the PTY capture fails or the libghostty-vt parse
/// rejects the captured bytes.
pub fn run(args: ExecArgs) -> Result<()> {
    let captured = pty::run(&args.cmd, args.cols, args.rows)?;
    let grid = vt::parse(&captured, args.cols as usize, args.rows as usize)?;
    eprintln!(
        "captured {} bytes, parsed {}x{} grid",
        captured.len(),
        grid.rows,
        grid.cols,
    );
    Ok(())
}
