//! Real PTY capture for command execution.
//!
//! Programs spawned through this module see `isatty=true`, so they emit
//! the full ANSI surface (colors, cursor moves, OSC sequences, BEL, and
//! every escape an actual terminal would receive).

use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::Read;

/// Run `cmd` in a fresh PTY of the given size and return every byte the
/// child wrote to its slave end.
///
/// Blocks until the child exits.
///
/// # Errors
///
/// Returns `Err` when the PTY cannot be allocated, the command cannot be
/// spawned, the master read fails, or the child cannot be waited on.
pub fn run(cmd: &[String], cols: u16, rows: u16) -> Result<Vec<u8>> {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("opening pty")?;

    let mut command = CommandBuilder::new(&cmd[0]);
    if cmd.len() > 1 {
        command.args(&cmd[1..]);
    }
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("FORCE_COLOR", "1");
    command.env("CLICOLOR_FORCE", "1");

    let mut child = pty.slave.spawn_command(command).context("spawning child")?;
    drop(pty.slave);

    let mut reader = pty.master.try_clone_reader().context("cloning reader")?;
    drop(pty.master);

    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    child.wait().context("waiting for child")?;
    Ok(out)
}
