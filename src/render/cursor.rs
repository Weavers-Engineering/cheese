//! Cursor rendering.
//!
//! v0.1 ships block cursor only. Drawn last so the cursor sits on top
//! of any cell glyph at its position. Color comes from the theme's
//! `cursor` field.

use tiny_skia::{Paint, Pixmap, Rect, Transform};

use crate::theme::Theme;

/// Paint a block cursor at `(x, y)` with the cell-sized footprint.
pub fn draw(pixmap: &mut Pixmap, x: u32, y: u32, cell_w: u32, cell_h: u32, theme: &Theme) {
    let color = Theme::parse_color(&theme.cursor);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = false;
    let rect = match Rect::from_xywh(x as f32, y as f32, cell_w as f32, cell_h as f32) {
        Some(r) => r,
        None => return,
    };
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}
