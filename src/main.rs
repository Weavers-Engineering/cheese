//! `cheese`: say cheese, get a screenshot of your terminal.
//!
//! v0.0.1 ships the chassis: clap-driven subcommands, version, exit shape.
//! v0.1 adds `cheese exec <cmd>`: run a command in a real PTY, capture
//! the full ANSI stream, render to PNG with operator-grade fidelity.
//! v0.2 adds `cheese capture`: pull the running terminal pane's
//! contents via terminal-RPC (iTerm2, Kitty, Ghostty).

use anyhow::Result;
use clap::{Parser, Subcommand};

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
        /// Output file (.png, .svg, .webp). Defaults to ./cheese.png.
        #[arg(short, long, default_value = "cheese.png")]
        output: String,
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
        Command::Exec { output, cmd } => {
            eprintln!("cheese exec: pty + vt100 + render not yet wired (v0.1).");
            eprintln!("  would run: {}", cmd.join(" "));
            eprintln!("  would write: {output}");
            std::process::exit(64);
        }
        Command::Capture => {
            eprintln!("cheese capture: terminal-RPC pane capture not yet wired (v0.2).");
            std::process::exit(64);
        }
    }
}
