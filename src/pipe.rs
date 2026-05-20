//! `cheese` pipe flow: read raw bytes from stdin, parse them through
//! the VT state machine, render, deliver.
//!
//! Used when cheese is invoked with no subcommand and no positional
//! command but stdin is piped (`cat foo.log | cheese`, here-docs,
//! programmatic feeds). The bytes are the picture: no PTY spawn, no
//! source introspection.
//!
//! Source tools that strip ANSI / collapse width when their stdout is
//! a pipe will produce a lossy render here. Use `cheese <cmd>` (the
//! default exec form) for tty-sensitive tools.

use anyhow::{Context, Result};
use std::io::Read;

use crate::exec::{RenderRequest, render_and_emit};

/// Drain stdin to EOF and render the captured bytes.
///
/// # Errors
///
/// Returns `Err` when stdin read fails or the underlying render
/// pipeline fails.
pub fn run(req: RenderRequest) -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .context("reading stdin")?;
    render_and_emit(&bytes, &req)
}
