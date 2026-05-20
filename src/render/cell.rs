//! Per-cell paint routine.
//!
//! Each cell turns into a background rect plus an optional shaped glyph
//! plus optional underline/strikethrough decorations. The cosmic-text
//! buffer is reused across cells and reshapes only the single character
//! we are about to paint.

use cosmic_text::{Attrs, Family, FontSystem, Shaping, Style, Weight};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

use crate::{
    render::font::{CellMetrics, FONT_FAMILY, GlyphScratch},
    theme::Theme,
    vt::{Cell, Color as CellColor},
};

/// Mutable per-render context threaded through every cell draw.
///
/// Holds the shared `FontSystem`, the scratch buffer + swash cache, and
/// the resolved cell metrics so the per-cell call sites stay short.
pub struct CellContext<'a> {
    pub pixmap: &'a mut Pixmap,
    pub font_system: &'a mut FontSystem,
    pub scratch: &'a mut GlyphScratch,
    pub metrics: CellMetrics,
    pub theme: &'a Theme,
}

/// Paint one cell into the pixmap at the given pixel origin.
pub fn draw(ctx: &mut CellContext<'_>, cell: Cell, x: u32, y: u32) {
    let fg = resolve_color(cell.fg, ctx.theme, FgOrBg::Fg);
    let bg = resolve_color(cell.bg, ctx.theme, FgOrBg::Bg);
    paint_bg(ctx.pixmap, x, y, ctx.metrics, cell.bg, bg);
    paint_glyph(ctx, cell, x, y, fg);
    paint_decorations(ctx.pixmap, cell, x, y, ctx.metrics, fg);
}

enum FgOrBg {
    Fg,
    Bg,
}

fn resolve_color(cell: CellColor, theme: &Theme, role: FgOrBg) -> Color {
    match cell {
        CellColor::Rgb(r, g, b) => Color::from_rgba8(r, g, b, 255),
        CellColor::Default => match role {
            FgOrBg::Fg => Theme::parse_color(&theme.foreground),
            FgOrBg::Bg => Theme::parse_color(&theme.background),
        },
    }
}

fn paint_bg(
    pixmap: &mut Pixmap,
    x: u32,
    y: u32,
    metrics: CellMetrics,
    bg_source: CellColor,
    bg: Color,
) {
    // Only fill explicitly-set backgrounds. Default bg is the canvas
    // fill; painting it again is a waste of cycles for the majority of
    // cells.
    if matches!(bg_source, CellColor::Default) {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(bg);
    paint.anti_alias = false;
    let rect = match Rect::from_xywh(
        x as f32,
        y as f32,
        metrics.width as f32,
        metrics.height as f32,
    ) {
        Some(r) => r,
        None => return,
    };
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

fn paint_glyph(ctx: &mut CellContext<'_>, cell: Cell, x: u32, y: u32, fg: Color) {
    if cell.ch == ' ' || cell.ch == '\0' {
        return;
    }
    let mut text_buf = [0u8; 8];
    let s = cell.ch.encode_utf8(&mut text_buf);

    let mut attrs = Attrs::new().family(Family::Name(FONT_FAMILY));
    if cell.bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if cell.italic {
        attrs = attrs.style(Style::Italic);
    }
    ctx.scratch
        .buffer
        .set_text(ctx.font_system, s, attrs, Shaping::Advanced);
    ctx.scratch
        .buffer
        .shape_until_scroll(ctx.font_system, false);

    let cell_w = ctx.metrics.width as f32;
    let fg_premul = fg.premultiply().to_color_u8();
    let pixmap_w = ctx.pixmap.width() as i32;
    let pixmap_h = ctx.pixmap.height() as i32;
    let pixmap_data = ctx.pixmap.data_mut();

    let runs: Vec<_> = ctx.scratch.buffer.layout_runs().collect();
    for run in runs {
        for layout_glyph in run.glyphs.iter() {
            let physical = layout_glyph.physical((0.0, 0.0), 1.0);
            let glyph_w = layout_glyph.w;
            // Center the glyph in the cell horizontally; baseline-align
            // it against the line baseline cosmic-text reports.
            let dx = x as f32 + ((cell_w - glyph_w) * 0.5).max(0.0);
            let dy = y as f32 + run.line_y;

            ctx.scratch.swash.with_pixels(
                ctx.font_system,
                physical.cache_key,
                cosmic_text::Color::rgba(255, 255, 255, 255),
                |gx, gy, gcolor| {
                    let alpha = gcolor.a();
                    if alpha == 0 {
                        return;
                    }
                    let px = (dx as i32) + physical.x + gx;
                    let py = (dy as i32) + physical.y + gy;
                    if px < 0 || py < 0 || px >= pixmap_w || py >= pixmap_h {
                        return;
                    }
                    let idx = ((py as usize) * (pixmap_w as usize) + (px as usize)) * 4;
                    if idx + 3 >= pixmap_data.len() {
                        return;
                    }
                    let a = alpha as u32;
                    let inv = 255 - a;
                    let src_r = fg_premul.red() as u32 * a / 255;
                    let src_g = fg_premul.green() as u32 * a / 255;
                    let src_b = fg_premul.blue() as u32 * a / 255;
                    let dst_r = pixmap_data[idx] as u32;
                    let dst_g = pixmap_data[idx + 1] as u32;
                    let dst_b = pixmap_data[idx + 2] as u32;
                    let dst_a = pixmap_data[idx + 3] as u32;
                    pixmap_data[idx] = (src_r + dst_r * inv / 255) as u8;
                    pixmap_data[idx + 1] = (src_g + dst_g * inv / 255) as u8;
                    pixmap_data[idx + 2] = (src_b + dst_b * inv / 255) as u8;
                    pixmap_data[idx + 3] = (a + dst_a * inv / 255).min(255) as u8;
                },
            );
        }
    }
}

fn paint_decorations(
    pixmap: &mut Pixmap,
    cell: Cell,
    x: u32,
    y: u32,
    metrics: CellMetrics,
    fg: Color,
) {
    if !cell.underline && !cell.strikethrough {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(fg);
    paint.anti_alias = false;
    let thickness = (metrics.height as f32 / 16.0).max(1.0);
    let cell_w = metrics.width as f32;
    if cell.underline {
        let uy = y as f32 + metrics.height as f32 - thickness - 1.0;
        if let Some(rect) = Rect::from_xywh(x as f32, uy, cell_w, thickness) {
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
    if cell.strikethrough {
        let sy = y as f32 + metrics.height as f32 * 0.55;
        if let Some(rect) = Rect::from_xywh(x as f32, sy, cell_w, thickness) {
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
}
