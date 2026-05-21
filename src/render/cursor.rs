//! Cursor rendering.
//!
//! v0.1 ships block cursor only. Drawn last so the cursor sits on top
//! of any cell glyph at its position. Color comes from the theme's
//! `cursor` field, never via inversion against the cell background; the
//! theme is the single source of truth for cursor visibility.
//!
//! The `CursorStyle` enum reserves slots for v0.2's `Underline` (a 2px
//! line along the cell's bottom edge) and `Bar` (a 2px line along the
//! cell's left edge). v0.1 plumbs the style through but the VT grid
//! always reports `Block`, so the other variants are forward-compat
//! shapes waiting on `vt::Grid::cursor_style`.

use tiny_skia::{Paint, Pixmap, Rect, Transform};

use crate::theme::Theme;

/// Visual style for the rendered cursor.
///
/// `Block` fills the cell with the theme's cursor color. `Underline`
/// paints a 2px line along the cell's bottom edge. `Bar` paints a 2px
/// line along the cell's left edge. v0.1 only ever picks `Block`; the
/// other variants land in v0.2 when the VT grid carries cursor shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    Bar,
}

/// Thickness of the underline / bar cursor in pixels.
const LINE_THICKNESS: f32 = 2.0;

/// Paint a cursor at `(x, y)` over a cell of `cell_w` x `cell_h`.
pub fn draw(
    pixmap: &mut Pixmap,
    x: u32,
    y: u32,
    cell_w: u32,
    cell_h: u32,
    theme: &Theme,
    style: CursorStyle,
) {
    let color = Theme::parse_color(&theme.cursor);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = false;

    let (rx, ry, rw, rh) = match style {
        CursorStyle::Block => (x as f32, y as f32, cell_w as f32, cell_h as f32),
        CursorStyle::Underline => (
            x as f32,
            y as f32 + cell_h as f32 - LINE_THICKNESS,
            cell_w as f32,
            LINE_THICKNESS,
        ),
        CursorStyle::Bar => (x as f32, y as f32, LINE_THICKNESS, cell_h as f32),
    };

    if let Some(rect) = Rect::from_xywh(rx, ry, rw, rh) {
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}
