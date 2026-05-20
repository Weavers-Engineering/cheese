//! `cheese` pipe flow: when stdin is a pipe, capture the sibling
//! command IMMEDIATELY at startup and re-execute it inside a real PTY
//! so the source sees `isatty(stdout)=true` (colors + width detection
//! intact). Fall back to rendering raw piped bytes when sibling
//! discovery fails (file redirection, complex pipelines, sources that
//! already exited).
//!
//! Detection shells out to `ps -A -o pid=,pgid=,args=` and filters by
//! process group. `ps` reads `KERN_PROCARGS2` on macOS, so the full
//! argv comes back intact. An earlier `sysinfo`-based implementation
//! dropped argv to argv\[0\] only on macOS, which broke `isd ps`
//! into `isd` (no subcommand).

use anyhow::{Context, Result};
use std::io::Read;
use std::process::Command;

use crate::exec::{self, ExecArgs, RenderRequest, render_and_emit};

/// Drain stdin and render the captured bytes, or re-exec the sibling
/// command in a PTY when we can identify it.
///
/// `sibling` is captured by `capture_sibling_argv()` at the very top of
/// `main()` so the discovery happens before any clap / setup overhead,
/// maximizing the window before fast sources like `isd ps` exit.
///
/// # Errors
///
/// Returns `Err` when stdin read fails or the underlying render or
/// re-exec pipeline fails.
pub fn run(req: RenderRequest, sibling: Option<Vec<String>>) -> Result<()> {
    if let Some(cmd) = sibling {
        eprintln!("cheese: re-running pipe sibling in PTY: {}", cmd.join(" "));
        drain_stdin_silent();
        return exec::run(ExecArgs { cmd, render: req });
    }
    eprintln!("cheese: no single pipe sibling found, rendering raw bytes");
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .context("reading stdin")?;
    render_and_emit(&bytes, &req)
}

/// Walk processes in our process group via `ps -A`. Return argv when
/// exactly one non-cheese sibling exists in the same pgrp. Called once
/// at the top of `main()` before clap parsing so the race against
/// fast sources is as small as possible.
///
/// Returns `None` when 0 or >=2 siblings live in the pgrp, when `ps`
/// cannot be run, or when shell-word splitting of `ps`'s args field
/// fails.
pub fn capture_sibling_argv() -> Option<Vec<String>> {
    let my_pid: i64 = std::process::id() as i64;
    let my_pgrp: i64 = unsafe { getpgrp() as i64 };

    // Spawn `ps` and grab its PID before reading its output: ps is
    // itself in our pgrp during the scan, so it would show up as a
    // pipe sibling and clobber the real source.
    let child = Command::new("ps")
        .args(["-A", "-o", "pid=,pgid=,args="])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let ps_pid: i64 = child.id() as i64;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;

    let mut found: Option<Vec<String>> = None;
    let mut count = 0_usize;
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        let mut it = trimmed
            .splitn(3, char::is_whitespace)
            .filter(|s| !s.is_empty());
        let pid_s = match it.next() {
            Some(s) => s,
            None => continue,
        };
        let pgid_s = match it.next() {
            Some(s) => s,
            None => continue,
        };
        let args = it.next().unwrap_or("");
        let pid: i64 = match pid_s.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let pgid: i64 = match pgid_s.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if pid == my_pid || pid == ps_pid || pgid != my_pgrp {
            continue;
        }
        // Belt-and-suspenders: also skip any other `ps` invocation,
        // in case the PID-based exclusion misses (e.g. ps re-exec
        // itself, fork race).
        if argv_command_is_ps(args) {
            continue;
        }
        count += 1;
        if count > 1 {
            return None;
        }
        let argv = shell_words::split(args).ok()?;
        if argv.is_empty() {
            return None;
        }
        found = Some(argv);
    }
    found
}

/// True when the args field's first token (the command) is a `ps`
/// invocation (`ps`, `/bin/ps`, `/usr/bin/ps`, etc.). Catches the case
/// where our own ps shellout shows up under a different PID than the
/// one we recorded (fork timing, kernel reuse).
fn argv_command_is_ps(args: &str) -> bool {
    let first = args.split_whitespace().next().unwrap_or("");
    let basename = first.rsplit('/').next().unwrap_or(first);
    basename == "ps"
}

// `getpgrp(3)`. Always returns the calling process's pgrp; never
// fails. Pulling `nix` for this alone was over budget.
unsafe extern "C" {
    fn getpgrp() -> i32;
}

/// Read stdin to EOF and discard. Used after we found a pipe sibling
/// so the source can finish writing without SIGPIPE while we set up
/// its replacement PTY run.
fn drain_stdin_silent() {
    let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
}
