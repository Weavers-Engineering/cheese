//! Drive `alacritty_terminal` against a captured byte stream and
//! snapshot the resulting screen into a plain `Grid` suitable for the
//! renderer.
//!
//! cheese keeps its own `Grid` / `Cell` / `Color` shape so the renderer
//! is decoupled from the parser crate. The mapping from
//! `alacritty_terminal`'s palette-aware color model to cheese's
//! resolved RGB lives in `resolve_color` below.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{
    Color as AnsiColor, NamedColor, Processor, Rgb as AnsiRgb, StdSyncHandler,
};
use anyhow::{Context, Result};

/// Final grid extracted from a captured byte stream.
#[derive(Debug, Clone)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    /// Row-major, length `rows * cols`.
    pub cells: Vec<Cell>,
    /// `(row, col)` when the cursor is on-screen.
    pub cursor: Option<(usize, usize)>,
    /// Number of rows from the top that carry non-blank content
    /// (max non-blank row index + 1). Always at least 1.
    /// The renderer uses this to crop trailing empty rows.
    pub used_rows: usize,
    /// Number of columns from the left that carry non-blank content
    /// (max non-blank column index + 1). Always at least 1.
    /// The renderer uses this to crop trailing empty columns so that
    /// piped input rendered at a generously wide virtual terminal does
    /// not pad the canvas with empty cells.
    pub used_cols: usize,
}

/// A single cell with character + resolved colors + style flags.
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

/// Resolved cell color. `Default` means the cell carried the terminal
/// default; the renderer maps that to the theme's fg / bg.
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Default,
    Rgb(u8, u8, u8),
}

/// Terminal dimensions wrapper. `alacritty_terminal::Term::new` is
/// generic over `&dyn Dimensions`.
struct GridDims {
    cols: usize,
    rows: usize,
}

impl Dimensions for GridDims {
    fn columns(&self) -> usize {
        self.cols
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn total_lines(&self) -> usize {
        self.rows
    }
}

/// Feed `bytes` through `alacritty_terminal` and snapshot the final grid.
///
/// `cols` and `rows` must match the PTY size the bytes were captured at.
///
/// # Errors
///
/// Returns `Err` when dimensions cannot fit `u16`. The parser itself is
/// infallible.
pub fn parse(bytes: &[u8], cols: usize, rows: usize) -> Result<Grid> {
    u16::try_from(cols).context("cols overflow u16")?;
    u16::try_from(rows).context("rows overflow u16")?;

    let dims = GridDims { cols, rows };
    let config = Config::default();
    let mut term: Term<VoidListener> = Term::new(config, &dims, VoidListener);
    let mut parser: Processor<StdSyncHandler> = Processor::new();
    parser.advance(&mut term, bytes);

    let grid_ref = term.grid();
    let mut cells = Vec::with_capacity(rows * cols);
    for row_idx in 0..rows {
        for col_idx in 0..cols {
            let point = Point::new(Line(row_idx as i32), Column(col_idx));
            let alac_cell = &grid_ref[point];
            cells.push(Cell {
                ch: alac_cell.c,
                fg: resolve_color(alac_cell.fg),
                bg: resolve_color(alac_cell.bg),
                bold: alac_cell.flags.contains(Flags::BOLD),
                italic: alac_cell.flags.contains(Flags::ITALIC),
                underline: alac_cell
                    .flags
                    .intersects(Flags::UNDERLINE | Flags::DOUBLE_UNDERLINE | Flags::UNDERCURL),
                strikethrough: alac_cell.flags.contains(Flags::STRIKEOUT),
            });
        }
    }

    // Honor DECTCEM: when the captured stream issued `CSI ?25l` (cursor
    // hide) and never re-enabled it, drop the cursor entirely. Inline
    // TUIs in their NORMAL / navigation mode commonly hide the cursor;
    // without this check, cheese renders a stray block over whatever
    // glyph happens to sit at the tracked position.
    let cursor_visible = term.mode().contains(TermMode::SHOW_CURSOR);
    let cursor_point = grid_ref.cursor.point;
    let (cur_row, cur_col) = (cursor_point.line.0 as usize, cursor_point.column.0);
    let cursor = if cursor_visible && cur_row < rows && cur_col < cols {
        Some((cur_row, cur_col))
    } else {
        None
    };

    let used_rows = compute_used_rows(&cells, rows, cols);
    let used_cols = compute_used_cols(&cells, rows, cols);

    Ok(Grid {
        rows,
        cols,
        cells,
        cursor,
        used_rows,
        used_cols,
    })
}

/// Find the last row that contains any non-space character, +1.
/// Clamps to at least 1 so the canvas is never zero-height.
fn compute_used_rows(cells: &[Cell], rows: usize, cols: usize) -> usize {
    for row in (0..rows).rev() {
        let start = row * cols;
        let end = start + cols;
        if cells[start..end]
            .iter()
            .any(|c| c.ch != ' ' && c.ch != '\0')
        {
            return row + 1;
        }
    }
    1
}

/// Find the last column that carries non-space content anywhere in
/// the grid, +1. Clamps to at least 1 so the canvas is never
/// zero-width.
fn compute_used_cols(cells: &[Cell], rows: usize, cols: usize) -> usize {
    let mut max_col_plus_1 = 1;
    for row in 0..rows {
        let start = row * cols;
        for col in (0..cols).rev() {
            let c = cells[start + col];
            if c.ch != ' ' && c.ch != '\0' {
                if col + 1 > max_col_plus_1 {
                    max_col_plus_1 = col + 1;
                }
                break;
            }
        }
    }
    max_col_plus_1
}

/// Map alacritty's tri-variant color to cheese's resolved `Color`.
///
/// `Named` palette indexes are converted via the standard xterm 16-color
/// table. `Indexed` 0..16 reuses that table; 16..232 walks the 6x6x6
/// cube; 232..256 walks the grayscale ramp. `Spec` passes through.
fn resolve_color(c: AnsiColor) -> Color {
    match c {
        AnsiColor::Named(name) => named_to_rgb(name),
        AnsiColor::Indexed(idx) => indexed_to_rgb(idx),
        AnsiColor::Spec(AnsiRgb { r, g, b }) => Color::Rgb(r, g, b),
    }
}

fn named_to_rgb(name: NamedColor) -> Color {
    match name {
        NamedColor::Foreground => Color::Default,
        NamedColor::Background => Color::Default,
        NamedColor::Cursor => Color::Default,
        NamedColor::DimForeground | NamedColor::BrightForeground => Color::Default,
        NamedColor::DimBlack | NamedColor::Black => Color::Rgb(0x15, 0x16, 0x1e),
        NamedColor::DimRed | NamedColor::Red => Color::Rgb(0xf7, 0x76, 0x8e),
        NamedColor::DimGreen | NamedColor::Green => Color::Rgb(0x9e, 0xce, 0x6a),
        NamedColor::DimYellow | NamedColor::Yellow => Color::Rgb(0xe0, 0xaf, 0x68),
        NamedColor::DimBlue | NamedColor::Blue => Color::Rgb(0x7a, 0xa2, 0xf7),
        NamedColor::DimMagenta | NamedColor::Magenta => Color::Rgb(0xbb, 0x9a, 0xf7),
        NamedColor::DimCyan | NamedColor::Cyan => Color::Rgb(0x7d, 0xcf, 0xff),
        NamedColor::DimWhite | NamedColor::White => Color::Rgb(0xa9, 0xb1, 0xd6),
        NamedColor::BrightBlack => Color::Rgb(0x41, 0x48, 0x68),
        NamedColor::BrightRed => Color::Rgb(0xf7, 0x76, 0x8e),
        NamedColor::BrightGreen => Color::Rgb(0x9e, 0xce, 0x6a),
        NamedColor::BrightYellow => Color::Rgb(0xe0, 0xaf, 0x68),
        NamedColor::BrightBlue => Color::Rgb(0x7a, 0xa2, 0xf7),
        NamedColor::BrightMagenta => Color::Rgb(0xbb, 0x9a, 0xf7),
        NamedColor::BrightCyan => Color::Rgb(0x7d, 0xcf, 0xff),
        NamedColor::BrightWhite => Color::Rgb(0xc0, 0xca, 0xf5),
    }
}

fn indexed_to_rgb(idx: u8) -> Color {
    if idx < 16 {
        let named = match idx {
            0 => NamedColor::Black,
            1 => NamedColor::Red,
            2 => NamedColor::Green,
            3 => NamedColor::Yellow,
            4 => NamedColor::Blue,
            5 => NamedColor::Magenta,
            6 => NamedColor::Cyan,
            7 => NamedColor::White,
            8 => NamedColor::BrightBlack,
            9 => NamedColor::BrightRed,
            10 => NamedColor::BrightGreen,
            11 => NamedColor::BrightYellow,
            12 => NamedColor::BrightBlue,
            13 => NamedColor::BrightMagenta,
            14 => NamedColor::BrightCyan,
            _ => NamedColor::BrightWhite,
        };
        return named_to_rgb(named);
    }
    if idx < 232 {
        let n = idx - 16;
        let r = (n / 36) % 6;
        let g = (n / 6) % 6;
        let b = n % 6;
        let lift = |v: u8| -> u8 { if v == 0 { 0 } else { 55 + 40 * v } };
        return Color::Rgb(lift(r), lift(g), lift(b));
    }
    let v = 8 + (idx - 232) * 10;
    Color::Rgb(v, v, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_input_yields_blank_grid() {
        let grid = parse(b"", 4, 2).unwrap();
        assert_eq!(grid.rows, 2);
        assert_eq!(grid.cols, 4);
        assert_eq!(grid.cells.len(), 8);
        for c in &grid.cells {
            assert_eq!(c.ch, ' ');
        }
    }

    #[test]
    fn parse_hello_lands_in_first_row() {
        let grid = parse(b"Hello", 10, 2).unwrap();
        let row0: String = grid.cells[..10].iter().map(|c| c.ch).collect();
        assert!(row0.starts_with("Hello"));
    }

    #[test]
    fn parse_red_sgr_resolves_to_red_rgb() {
        let grid = parse(b"\x1b[31mR", 4, 1).unwrap();
        match grid.cells[0].fg {
            Color::Rgb(0xf7, 0x76, 0x8e) => {}
            other => panic!("expected tokyo-night red, got {:?}", other),
        }
    }

    #[test]
    fn used_rows_crops_to_last_non_blank_row() {
        let grid = parse(b"line1\r\nline2\r\n", 10, 20).unwrap();
        assert_eq!(grid.used_rows, 2);
        assert_eq!(grid.rows, 20);
    }

    #[test]
    fn used_rows_blank_input_is_one() {
        let grid = parse(b"", 4, 8).unwrap();
        assert_eq!(grid.used_rows, 1);
    }

    #[test]
    fn cursor_visible_by_default() {
        // Default DECTCEM is on; the parsed grid should carry the
        // tracked position.
        let grid = parse(b"hi", 4, 2).unwrap();
        assert!(grid.cursor.is_some(), "default cursor should be visible");
    }

    #[test]
    fn dec_25l_hides_cursor() {
        // `CSI ?25l` issued by the TUI must drop the cursor from the
        // rendered grid. This mirrors what every inline TUI in NORMAL
        // mode does so cheese stops painting a stray block.
        let grid = parse(b"hi\x1b[?25l", 4, 2).unwrap();
        assert_eq!(grid.cursor, None, "?25l should hide the cursor");
    }

    #[test]
    fn dec_25h_after_25l_restores_cursor() {
        // Toggling back on (INSERT mode in a vim-modal picker, say)
        // brings the rendered cursor back.
        let grid = parse(b"hi\x1b[?25l\x1b[?25h", 4, 2).unwrap();
        assert!(grid.cursor.is_some(), "?25h should re-show the cursor");
    }
}
