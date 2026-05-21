//! Window-chrome rendering (the strip above the cell region).
//!
//! Phase 5 ships `Chrome::Mac`: a 60px strip carrying the three
//! traffic-light circles (close / minimize / maximize) plus a 1px
//! hairline that separates the chrome from the cells. The strip uses
//! the theme's background color so the window looks like a single
//! surface; the hairline uses the theme's selection color for a soft
//! divider that reads on both dark and light themes.
//!
//! `Chrome::None` paints nothing: the renderer simply allocates a
//! `chrome_h` of 0 and the cell region starts at the top padding.

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

use crate::theme::Theme;

/// Height of the Mac chrome strip in pixels.
pub const MAC_CHROME_HEIGHT: u32 = 60;

/// Radius of each traffic-light circle in pixels.
pub const MAC_CIRCLE_RADIUS: f32 = 7.0;

/// Distance from the canvas left edge to the center of the leftmost
/// circle, in pixels.
pub const MAC_LEFT_PAD: f32 = 20.0;

/// Gap between adjacent circle edges along the x axis, in pixels.
/// Center-to-center spacing is `2 * radius + gap`.
pub const MAC_CIRCLE_GAP: f32 = 8.0;

/// Traffic-light colors. Matches the standard macOS palette so the
/// chrome reads as familiar regardless of the underlying theme.
const COLOR_RED: (u8, u8, u8) = (0xff, 0x5f, 0x56);
const COLOR_YELLOW: (u8, u8, u8) = (0xff, 0xbd, 0x2e);
const COLOR_GREEN: (u8, u8, u8) = (0x27, 0xc9, 0x3f);

/// Compute the pixel height reserved for the given chrome style.
///
/// Returns 0 for `Chrome::None`. Used by the renderer to size the
/// pixmap before any painting happens.
pub fn height(chrome: crate::exec::Chrome) -> u32 {
    match chrome {
        crate::exec::Chrome::None => 0,
        crate::exec::Chrome::Mac => MAC_CHROME_HEIGHT,
    }
}

/// Paint the Mac traffic-light chrome strip across the top of `pixmap`.
///
/// The strip spans the full canvas width and `MAC_CHROME_HEIGHT` pixels
/// of height. The background uses `theme.background` (already painted
/// by the canvas fill, so we no-op the strip fill itself), and the
/// hairline divider uses `theme.selection`. The three circles use the
/// fixed macOS palette.
pub fn draw_mac(pixmap: &mut Pixmap, theme: &Theme) {
    let w = pixmap.width();
    if w == 0 {
        return;
    }

    // The canvas fill in `render::draw` has already painted the strip
    // background. Skip re-painting it.

    // Centers of the three circles. Spacing is center-to-center:
    // 2 * radius + gap.
    let cy = MAC_CHROME_HEIGHT as f32 / 2.0;
    let step = 2.0 * MAC_CIRCLE_RADIUS + MAC_CIRCLE_GAP;
    let cx_red = MAC_LEFT_PAD + MAC_CIRCLE_RADIUS;
    let cx_yellow = cx_red + step;
    let cx_green = cx_yellow + step;

    draw_circle(pixmap, cx_red, cy, MAC_CIRCLE_RADIUS, COLOR_RED);
    draw_circle(pixmap, cx_yellow, cy, MAC_CIRCLE_RADIUS, COLOR_YELLOW);
    draw_circle(pixmap, cx_green, cy, MAC_CIRCLE_RADIUS, COLOR_GREEN);

    // 1px hairline divider at the bottom of the strip.
    let hairline = Theme::parse_color(&theme.selection);
    let mut paint = Paint::default();
    paint.set_color(hairline);
    paint.anti_alias = false;
    if let Some(rect) = Rect::from_xywh(0.0, (MAC_CHROME_HEIGHT - 1) as f32, w as f32, 1.0) {
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

/// Fill an antialiased circle of `radius` centered at `(cx, cy)`.
fn draw_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, radius: f32, rgb: (u8, u8, u8)) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, radius);
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(rgb.0, rgb.1, rgb.2, 255));
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    /// Read the RGBA bytes at a given pixel from the pixmap.
    fn pixel_at(pixmap: &Pixmap, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let i = ((y * pixmap.width()) + x) as usize * 4;
        let data = pixmap.data();
        (data[i], data[i + 1], data[i + 2], data[i + 3])
    }

    #[test]
    fn mac_chrome_paints_three_traffic_lights_at_expected_centers() {
        // Canvas wide enough to clear all three circles plus their pad.
        let mut pixmap = Pixmap::new(200, MAC_CHROME_HEIGHT).expect("alloc pixmap");
        let theme = Theme::tokyo_night_dark();
        pixmap.fill(Theme::parse_color(&theme.background));
        draw_mac(&mut pixmap, &theme);

        // Compute the integer pixel center of each circle.
        let cy = MAC_CHROME_HEIGHT / 2;
        let step = (2.0 * MAC_CIRCLE_RADIUS + MAC_CIRCLE_GAP) as u32;
        let cx_red = (MAC_LEFT_PAD + MAC_CIRCLE_RADIUS) as u32;
        let cx_yellow = cx_red + step;
        let cx_green = cx_yellow + step;

        let (r, g, b, a) = pixel_at(&pixmap, cx_red, cy);
        assert_eq!(
            (r, g, b, a),
            (0xff, 0x5f, 0x56, 0xff),
            "red circle pixel mismatch at ({cx_red}, {cy})"
        );
        let (r, g, b, a) = pixel_at(&pixmap, cx_yellow, cy);
        assert_eq!(
            (r, g, b, a),
            (0xff, 0xbd, 0x2e, 0xff),
            "yellow circle pixel mismatch at ({cx_yellow}, {cy})"
        );
        let (r, g, b, a) = pixel_at(&pixmap, cx_green, cy);
        assert_eq!(
            (r, g, b, a),
            (0x27, 0xc9, 0x3f, 0xff),
            "green circle pixel mismatch at ({cx_green}, {cy})"
        );
    }

    #[test]
    fn mac_chrome_paints_hairline_in_theme_selection_color() {
        let mut pixmap = Pixmap::new(64, MAC_CHROME_HEIGHT + 8).expect("alloc pixmap");
        let theme = Theme::tokyo_night_dark();
        pixmap.fill(Theme::parse_color(&theme.background));
        draw_mac(&mut pixmap, &theme);

        // Theme selection #283457. Sample the hairline at a column well
        // clear of any circle.
        let y = MAC_CHROME_HEIGHT - 1;
        let (r, g, b, a) = pixel_at(&pixmap, 60, y);
        assert_eq!(
            (r, g, b, a),
            (0x28, 0x34, 0x57, 0xff),
            "hairline pixel mismatch at column 60 row {y}"
        );
    }

    #[test]
    fn chrome_height_is_zero_for_none_and_sixty_for_mac() {
        assert_eq!(height(crate::exec::Chrome::None), 0);
        assert_eq!(height(crate::exec::Chrome::Mac), 60);
    }
}
