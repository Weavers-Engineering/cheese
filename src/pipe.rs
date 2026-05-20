//! `cheese` pipe flow: read raw bytes from stdin, parse them through
//! the VT state machine, render, deliver.
//!
//! Used when cheese is invoked without a subcommand and stdin is not a
//! tty (i.e. something is piped into it). No PTY spawn, no synthetic
//! prompt, no command label: the bytes are the picture.

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
