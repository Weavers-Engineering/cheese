//! `cheese`: say cheese, get a screenshot of your terminal.
//!
//! Two entry points:
//! - `cheese exec <cmd>`: spawn `<cmd>` in a real PTY, capture, render.
//! - `... | cheese`: read piped stdin, render. No subcommand needed.

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::io::IsTerminal;
use terminal_size::{Height, Width, terminal_size};

use cheese::exec::{self, Chrome, ExecArgs, RenderRequest};
use cheese::pipe;

const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 40;

/// Take a screenshot of your terminal.
///
/// Pixel-perfect renders of command output, with your font and your
/// theme. Pipe data in or use `exec` to spawn a command in a PTY.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Render knobs used in pipe mode. Ignored when a subcommand is
    /// chosen; the subcommand carries its own copy.
    #[command(flatten)]
    render: RenderFlags,
}

/// Render knobs shared by both entry points.
#[derive(Args, Debug, Clone)]
struct RenderFlags {
    /// Write the PNG to this path. Defaults to copying the image to
    /// the system clipboard.
    #[arg(short, long)]
    output: Option<String>,
    /// Virtual terminal columns. Defaults to your terminal's width
    /// (or 120 when cheese can't read a tty).
    #[arg(short = 'c', long)]
    cols: Option<u16>,
    /// Virtual terminal rows. Defaults to your terminal's height
    /// (or 40 when cheese can't read a tty).
    #[arg(short = 'r', long)]
    rows: Option<u16>,
    /// Font size in points.
    #[arg(short = 's', long, default_value_t = 14.0)]
    font_size: f32,
    /// Inner padding around the cell region in pixels.
    #[arg(long, default_value_t = 24)]
    padding: u32,
    /// Window chrome style (Phase 5+).
    #[arg(long, value_enum, default_value_t = Chrome::None)]
    chrome: Chrome,
    /// Theme name. v0.1 only ships `tokyo-night-dark`.
    #[arg(long, default_value = "tokyo-night-dark")]
    theme: String,
    /// Skip the drop shadow under the chrome (Phase 5+).
    #[arg(long, default_value_t = false)]
    no_shadow: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a command in a real PTY and render its output to an image.
    Exec {
        #[command(flatten)]
        flags: RenderFlags,
        /// The command and arguments to run.
        #[arg(required = true, trailing_var_arg = true)]
        cmd: Vec<String>,
    },
    /// Screenshot the current terminal pane via terminal-specific RPC.
    /// Available in v0.2+.
    Capture,
}

fn main() -> Result<()> {
    // Capture sibling argv BEFORE any other work. The race against
    // fast pipe sources (`isd ps` finishes in <100ms) is real, so we
    // shell out to `ps` first thing and only then parse our own args.
    let sibling = if std::io::stdin().is_terminal() {
        None
    } else {
        pipe::capture_sibling_argv()
    };

    let cli = Cli::parse();
    match cli.command {
        Some(Command::Exec { flags, cmd }) => exec::run(ExecArgs {
            cmd,
            render: build_request(flags),
        }),
        Some(Command::Capture) => {
            eprintln!("cheese capture: terminal-RPC pane capture not yet wired (v0.2).");
            std::process::exit(64);
        }
        None => {
            if std::io::stdin().is_terminal() {
                eprintln!(
                    "cheese: nothing to render. Pipe a command into me (`isd ps | cheese`) \
                     or use `cheese exec <cmd>`. Run `cheese --help` for options."
                );
                std::process::exit(64);
            }
            pipe::run(build_request(cli.render), sibling)
        }
    }
}

/// Materialize a `RenderRequest` from the CLI `RenderFlags`, filling in
/// detected terminal dimensions when the operator didn't override them.
fn build_request(flags: RenderFlags) -> RenderRequest {
    let (cols, rows) = detect_size(flags.cols, flags.rows);
    RenderRequest {
        cols,
        rows,
        output: flags.output.map(Into::into),
        font_size: flags.font_size,
        padding: flags.padding,
        chrome: flags.chrome,
        theme: flags.theme,
        no_shadow: flags.no_shadow,
    }
}

/// Read the controlling terminal's dimensions, falling back to the
/// hard-coded defaults when no tty is attached (CI, pipe, daemon).
fn detect_size(explicit_cols: Option<u16>, explicit_rows: Option<u16>) -> (u16, u16) {
    let detected = terminal_size();
    let detected_cols = match detected {
        Some((Width(w), _)) if w > 0 => w,
        _ => DEFAULT_COLS,
    };
    let detected_rows = match detected {
        Some((_, Height(h))) if h > 0 => h,
        _ => DEFAULT_ROWS,
    };
    (
        explicit_cols.unwrap_or(detected_cols),
        explicit_rows.unwrap_or(detected_rows),
    )
}
