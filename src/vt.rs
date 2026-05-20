//! Drive libghostty-vt against a captured byte stream and snapshot the
//! resulting screen into a plain `Grid` suitable for the renderer.
//!
//! Cells, colors, and cursor position are extracted via the render-state
//! iterator API (`RenderState` -> `Snapshot` -> `RowIterator` ->
//! `CellIterator`). Color resolution prefers the libghostty-vt-resolved
//! RGB pair (`fg_color` / `bg_color`); style flags (bold, italic,
//! underline, strikethrough) come from the per-cell `Style`.

use anyhow::{Context, Result};
use libghostty_vt::{
    Terminal, TerminalOptions,
    render::{CellIterator, RenderState, RowIterator},
    style::{RgbColor, Underline},
};

/// Final grid extracted from a libghostty-vt screen snapshot.
#[derive(Debug, Clone)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    /// Row-major, length `rows * cols`.
    pub cells: Vec<Cell>,
    /// `(row, col)` when the cursor is visible, otherwise `None`.
    pub cursor: Option<(usize, usize)>,
}

/// A single cell with its character + resolved colors + style flags.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

/// Color value mirrored from libghostty-vt's resolved color API.
///
/// libghostty-vt resolves palette + default colors against the active
/// terminal palette before handing them to us, so we only carry two
/// variants here.
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Default,
    Rgb(u8, u8, u8),
}

impl From<Option<RgbColor>> for Color {
    fn from(value: Option<RgbColor>) -> Self {
        match value {
            None => Color::Default,
            Some(c) => Color::Rgb(c.r, c.g, c.b),
        }
    }
}

/// Feed `bytes` through libghostty-vt and snapshot the final grid.
///
/// `cols` and `rows` must match the PTY size the bytes were captured at.
///
/// # Errors
///
/// Returns `Err` when libghostty-vt fails to allocate the terminal, when
/// feeding bytes fails, when the render-state update fails, or when any
/// per-cell accessor returns an error.
pub fn parse(bytes: &[u8], cols: usize, rows: usize) -> Result<Grid> {
    let cols_u16 = u16::try_from(cols).context("cols overflow u16")?;
    let rows_u16 = u16::try_from(rows).context("rows overflow u16")?;

    let mut terminal = Terminal::new(TerminalOptions {
        cols: cols_u16,
        rows: rows_u16,
        max_scrollback: 0,
    })
    .context("allocating libghostty-vt terminal")?;

    terminal.vt_write(bytes);

    let mut render_state = RenderState::new().context("allocating render state")?;
    let snapshot = render_state
        .update(&terminal)
        .context("updating render state from terminal")?;

    let cursor = snapshot
        .cursor_viewport()
        .context("reading cursor viewport")?
        .map(|c| (c.y as usize, c.x as usize));

    let mut row_iter = RowIterator::new().context("allocating row iterator")?;
    let mut row_iteration = row_iter
        .update(&snapshot)
        .context("starting row iteration")?;

    let mut cell_iter = CellIterator::new().context("allocating cell iterator")?;

    let mut cells: Vec<Cell> = vec![blank_cell(); rows * cols];
    let mut row_index = 0usize;

    while row_iteration.next().is_some() {
        if row_index >= rows {
            break;
        }
        let mut cell_iteration = cell_iter
            .update(&row_iteration)
            .context("starting cell iteration")?;
        let mut col_index = 0usize;
        while cell_iteration.next().is_some() {
            if col_index >= cols {
                break;
            }

            let style = cell_iteration.style().context("reading cell style")?;
            let fg: Color = cell_iteration
                .fg_color()
                .context("resolving fg color")?
                .into();
            let bg: Color = cell_iteration
                .bg_color()
                .context("resolving bg color")?
                .into();

            let len = cell_iteration
                .graphemes_len()
                .context("reading grapheme count")?;
            let ch = if len == 0 {
                ' '
            } else {
                let mut buf = vec!['\0'; len];
                cell_iteration
                    .graphemes_buf(&mut buf)
                    .context("copying graphemes")?;
                buf[0]
            };

            let underline = !matches!(style.underline, Underline::None);
            cells[row_index * cols + col_index] = Cell {
                ch,
                fg,
                bg,
                bold: style.bold,
                italic: style.italic,
                underline,
                strikethrough: style.strikethrough,
            };
            col_index += 1;
        }
        row_index += 1;
    }

    Ok(Grid {
        rows,
        cols,
        cells,
        cursor,
    })
}

fn blank_cell() -> Cell {
    Cell {
        ch: ' ',
        fg: Color::Default,
        bg: Color::Default,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
    }
}
