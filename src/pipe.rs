//! `cheese` pipe flow: when stdin is a pipe, try to re-execute the
//! sibling command in a real PTY so the source sees `isatty=true`
//! (colors + width detection intact). Fall back to rendering the raw
//! piped bytes when the introspection fails (complex pipelines, very
//! fast siblings that already exited, sources not exposed in the
//! process table).
//!
//! Tradeoff: simple pipes like `isd ps | cheese` run the source
//! command twice. Once via the shell-pipe (cheese discards those
//! bytes) and once via cheese's own PTY (the bytes we actually
//! render). Worth it for read-only commands; users should not pipe
//! state-mutating commands through cheese anyway.

use anyhow::{Context, Result};
use std::io::Read;

use crate::exec::{self, ExecArgs, RenderRequest, render_and_emit};

/// Drain stdin and render the captured bytes, or re-exec the sibling
/// command in a PTY when we can identify it.
///
/// # Errors
///
/// Returns `Err` when stdin read fails or the underlying render or
/// re-exec pipeline fails.
pub fn run(req: RenderRequest) -> Result<()> {
    if let Some(cmd) = detect_pipe_source() {
        drain_stdin_silent();
        return exec::run(ExecArgs { cmd, render: req });
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .context("reading stdin")?;
    render_and_emit(&bytes, &req)
}

/// Walk the process table for sibling processes (same process group
/// as cheese, different PID). Return argv of the unique sibling when
/// exactly one exists; otherwise `None` so the caller falls back to
/// raw-bytes rendering.
///
/// Single-sibling is the `<cmd> | cheese` shape every user means by
/// "pipe a command into cheese". Long pipelines (`a | b | c | cheese`)
/// would require reconstructing the chain order, which is fragile;
/// raw rendering handles them adequately.
fn detect_pipe_source() -> Option<Vec<String>> {
    let my_pid = std::process::id() as i32;
    let my_pgrp = nix::unistd::getpgrp().as_raw();

    let sys = sysinfo::System::new_all();
    let mut found: Option<Vec<String>> = None;
    let mut count = 0_usize;

    for proc in sys.processes().values() {
        let pid = proc.pid().as_u32() as i32;
        if pid == my_pid {
            continue;
        }
        let nix_pid = nix::unistd::Pid::from_raw(pid);
        let pgid = match nix::unistd::getpgid(Some(nix_pid)) {
            Ok(p) => p.as_raw(),
            Err(_) => continue,
        };
        if pgid != my_pgrp {
            continue;
        }
        count += 1;
        if count > 1 {
            return None;
        }
        let argv: Vec<String> = proc
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        if argv.is_empty() {
            return None;
        }
        found = Some(argv);
    }

    found
}

/// Read stdin to EOF and throw the bytes away. Used when we found a
/// pipe sibling: we still need to drain so the source does not
/// SIGPIPE while we are setting up its replacement PTY run.
fn drain_stdin_silent() {
    let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
}
