//! `cheese`: say cheese, get a screenshot of your terminal.
//!
//! v0.0.1 shipped the chassis: clap-driven subcommands, version, exit
//! shape. Phase 2 wires `cheese exec` to a real PTY + libghostty-vt
//! state extraction; the renderer lands in Phase 3.

use anyhow::Result;
use clap::{Parser, Subcommand};

use cheese::exec::{self, Chrome, ExecArgs};

/// Take a screenshot of your terminal.
///
/// Pixel-perfect renders of command output, with your font and your
/// theme. No more squinting at freeze captures.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a command in a real PTY and render its output to an image.
    Exec {
        /// Write the PNG to this path. Defaults to copying the image
        /// to the system clipboard.
        #[arg(short, long)]
        output: Option<String>,
        /// Virtual terminal columns.
        #[arg(short = 'c', long, default_value_t = 120)]
        cols: u16,
        /// Virtual terminal rows.
        #[arg(short = 'r', long, default_value_t = 40)]
        rows: u16,
        /// Font size in points (Phase 3+).
        #[arg(short = 's', long, default_value_t = 14.0)]
        font_size: f32,
        /// Inner padding around the cell region in pixels (Phase 3+).
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
        /// The command and arguments to run.
        #[arg(required = true, trailing_var_arg = true)]
        cmd: Vec<String>,
    },
    /// Screenshot the current terminal pane via terminal-specific RPC.
    /// Available in v0.2+.
    Capture,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Exec {
            output,
            cols,
            rows,
            font_size,
            padding,
            chrome,
            theme,
            no_shadow,
            cmd,
        } => exec::run(ExecArgs {
            cmd,
            cols,
            rows,
            output: output.map(Into::into),
            font_size,
            padding,
            chrome,
            theme,
            no_shadow,
        }),
        Command::Capture => {
            eprintln!("cheese capture: terminal-RPC pane capture not yet wired (v0.2).");
            std::process::exit(64);
        }
    }
}
