//! `cheese`: say cheese, get a screenshot of your terminal.
//!
//! Three ways to invoke:
//!
//! - `cheese <cmd>...`: spawn `<cmd>` in a real PTY, capture, render.
//!   Default form. Equivalent to the old `cheese exec` subcommand.
//! - `... | cheese`: render the raw piped bytes. Lossy when the
//!   source strips ANSI / collapses width on pipe.
//! - `cheese exec <cmd>`: explicit form, identical to `cheese <cmd>`.

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
/// theme. Pass a command to run in a real PTY, or pipe data in.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    flags: RenderFlags,

    /// Command and arguments to run in a PTY. Equivalent to
    /// `cheese exec -- <cmd>`. Mutually exclusive with subcommands.
    #[arg(trailing_var_arg = true)]
    cmd: Vec<String>,
}

/// Render knobs available at the top level and inside `exec`.
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
    /// Window chrome style. `none` (default) renders the cell region
    /// only; `mac` adds a 60px macOS-style strip with traffic-light
    /// circles above the cells.
    #[arg(long, value_enum, default_value_t = Chrome::None)]
    chrome: Chrome,
    /// Theme name. Bundled: `tokyo-night-dark`, `ayu-dark`.
    #[arg(long, default_value = "tokyo-night-dark")]
    theme: String,
    /// Skip the drop shadow under the chrome. v0.1 never paints a
    /// shadow; this flag is a forward-compat placeholder for v0.2.
    #[arg(long, default_value_t = false)]
    no_shadow: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Explicit exec form. Identical to `cheese <cmd>` at the top
    /// level; kept for backward compatibility and for the rare case
    /// where the command name collides with a subcommand.
    Exec {
        #[command(flatten)]
        flags: RenderFlags,
        /// The command and arguments to run.
        #[arg(required = true, trailing_var_arg = true)]
        cmd: Vec<String>,
    },
    /// Screenshot the current terminal pane via terminal-specific
    /// RPC. Available in v0.2+.
    Capture,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    dispatch(cli)
}

fn dispatch(cli: Cli) -> Result<()> {
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
            if !cli.cmd.is_empty() {
                return exec::run(ExecArgs {
                    cmd: cli.cmd,
                    render: build_request(cli.flags),
                });
            }
            if std::io::stdin().is_terminal() {
                eprintln!(
                    "cheese: nothing to render. Either `cheese <cmd>...` to run a command \
                     in a PTY, or pipe data into me (`cat log | cheese`). \
                     Run `cheese --help` for options."
                );
                std::process::exit(64);
            }
            pipe::run(build_request(cli.flags))
        }
    }
}

/// Materialize a `RenderRequest`, filling in detected terminal
/// dimensions when the operator didn't override them.
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
/// hard-coded defaults when no tty is attached.
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// `cheese isd ps` parses as a top-level positional command:
    /// no subcommand, both tokens land in `cmd`.
    #[test]
    fn parses_bare_positional_cmd() {
        let cli = Cli::try_parse_from(["cheese", "isd", "ps"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.cmd, vec!["isd", "ps"]);
        assert!(cli.flags.output.is_none());
    }

    /// `cheese "isd ps"` (one shell word, contains whitespace) lands
    /// as a single argv entry. `pty::run` shell-splits this via
    /// `$SHELL -c`.
    #[test]
    fn parses_quoted_single_arg_cmd() {
        let cli = Cli::try_parse_from(["cheese", "isd ps"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.cmd, vec!["isd ps"]);
    }

    /// Top-level flags compose with the positional command. Flags
    /// come before the command (`trailing_var_arg` captures
    /// everything after the first positional verbatim).
    #[test]
    fn parses_output_flag_then_cmd() {
        let cli = Cli::try_parse_from(["cheese", "-o", "out.png", "ls", "-la"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.flags.output.as_deref(), Some("out.png"));
        assert_eq!(cli.cmd, vec!["ls", "-la"]);
    }

    /// `cheese exec ls` still works as an explicit subcommand for
    /// backward compatibility and to disambiguate when a command
    /// name collides with a subcommand.
    #[test]
    fn parses_explicit_exec_subcommand() {
        let cli = Cli::try_parse_from(["cheese", "exec", "ls"]).unwrap();
        match cli.command {
            Some(Command::Exec { cmd, .. }) => assert_eq!(cmd, vec!["ls"]),
            other => panic!("expected Exec subcommand, got {:?}", other),
        }
    }

    /// `cheese capture` parses as the placeholder Capture subcommand
    /// (v0.2+).
    #[test]
    fn parses_capture_subcommand() {
        let cli = Cli::try_parse_from(["cheese", "capture"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Capture)));
    }

    /// `cheese -- capture` escapes the subcommand recognizer and
    /// treats `capture` as a binary to spawn. The `--` separator is
    /// required when running a real binary that happens to share a
    /// subcommand name.
    #[test]
    fn dashdash_escapes_subcommand_name() {
        let cli = Cli::try_parse_from(["cheese", "--", "capture"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.cmd, vec!["capture"]);
    }

    /// `cheese` with no args and no positional cmd. Parses cleanly
    /// with an empty cmd; runtime decides pipe-mode vs help based on
    /// `IsTerminal`.
    #[test]
    fn parses_bare_invocation_with_no_args() {
        let cli = Cli::try_parse_from(["cheese"]).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.cmd.is_empty());
    }

    /// Sanity check that clap's derive metadata compiles. Catches
    /// macro-generation regressions in the CLI struct shape.
    #[test]
    fn clap_metadata_is_well_formed() {
        Cli::command().debug_assert();
    }
}
