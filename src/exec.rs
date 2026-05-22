//! `cheese exec` flow: spawn the command in a PTY, parse the captured
//! bytes through the VT state machine, render the grid to a Pixmap, and
//! write the encoded PNG to disk (or push it to the clipboard).

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::{
    pty,
    render::{self, RenderOpts},
    theme::Theme,
    vt,
};

/// Optional window chrome around the cell region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Chrome {
    None,
    Mac,
}

/// Knobs shared by every cheese render entry point (exec and pipe).
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub cols: u16,
    pub rows: u16,
    /// `Some(path)` writes the PNG to disk; `None` copies the image to
    /// the system clipboard.
    pub output: Option<PathBuf>,
    pub font_size: f32,
    pub padding: u32,
    pub chrome: Chrome,
    pub theme: String,
    pub no_shadow: bool,
}

/// Arguments accepted by `cheese exec`.
#[derive(Debug, Clone)]
pub struct ExecArgs {
    pub cmd: Vec<String>,
    pub render: RenderRequest,
}

/// Spawn the child, parse its output, render the grid, write the PNG.
///
/// # Errors
///
/// Returns `Err` when the PTY capture fails, the VT parser rejects the
/// captured bytes, the theme name is unknown, the pixmap allocation
/// overflows, or the output path cannot be written.
pub fn run(args: ExecArgs) -> Result<()> {
    let captured = pty::run(&args.cmd, args.render.cols, args.render.rows)?;
    let stream = with_prompt(&args.cmd, &captured);
    render_and_emit(&stream, &args.render)
}

/// Parse `bytes` into a grid, render it, and deliver to the operator's
/// chosen sink (file path or clipboard).
///
/// Public so the pipe-mode entry point can call it without duplicating
/// the render+emit dance.
pub fn render_and_emit(bytes: &[u8], req: &RenderRequest) -> Result<()> {
    let grid = vt::parse(bytes, req.cols as usize, req.rows as usize)?;
    let theme = resolve_theme(&req.theme)?;
    let opts = RenderOpts {
        theme,
        font_size: req.font_size,
        padding: req.padding,
        chrome: req.chrome,
        no_shadow: req.no_shadow,
    };
    let pixmap = render::draw(&grid, &opts)?;
    match &req.output {
        Some(path) => {
            pixmap
                .save_png(path)
                .with_context(|| format!("writing png to {}", path.display()))?;
            eprintln!("wrote {}", path.display());
        }
        None => {
            copy_to_clipboard(&pixmap).context("copying to clipboard")?;
            eprintln!(
                "copied to clipboard ({} x {})",
                pixmap.width(),
                pixmap.height()
            );
        }
    }
    Ok(())
}

/// Push the rendered pixmap onto the system clipboard as an image.
///
/// tiny-skia stores premultiplied RGBA8. cheese always paints opaque
/// pixels (the theme background covers the canvas before any cell), so
/// premultiplied bytes equal straight RGBA and we hand them to
/// `arboard` as-is.
fn copy_to_clipboard(pixmap: &tiny_skia::Pixmap) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("opening clipboard")?;
    let image = arboard::ImageData {
        width: pixmap.width() as usize,
        height: pixmap.height() as usize,
        bytes: pixmap.data().to_vec().into(),
    };
    clipboard
        .set_image(image)
        .context("setting clipboard image")?;
    Ok(())
}

fn resolve_theme(name: &str) -> Result<Theme> {
    match name {
        "tokyo-night-dark" => Ok(Theme::tokyo_night_dark()),
        "ayu-dark" => Ok(Theme::ayu_dark()),
        other => Err(anyhow::anyhow!(
            "unknown theme {other:?}: bundled themes are tokyo-night-dark, ayu-dark"
        )),
    }
}

/// Prepend a synthetic shell prompt line to the captured stream so the
/// final render shows the command above its output.
fn with_prompt(cmd: &[String], captured: &[u8]) -> Vec<u8> {
    const PROMPT: &[u8] = b"\x1b[32m\xe2\x9d\xaf\x1b[0m ";
    let joined = cmd.join(" ");
    let mut out = Vec::with_capacity(PROMPT.len() + joined.len() + 2 + captured.len());
    out.extend_from_slice(PROMPT);
    out.extend_from_slice(joined.as_bytes());
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(captured);
    out
}
