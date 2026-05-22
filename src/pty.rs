//! Real PTY capture for command execution.
//!
//! Programs spawned through this module see `isatty=true`, so they emit
//! the full ANSI surface (colors, cursor moves, OSC sequences, BEL, and
//! every escape an actual terminal would receive).
//!
//! Two capture modes share the same loop:
//!
//! 1. Wait for the child to exit naturally (default for non-interactive
//!    commands).
//! 2. Snapshot on idle: once the child stops emitting bytes for
//!    `idle_after`, terminate it and return what's in the buffer. This
//!    is what makes `cheese isd ssh` (or any fzf/inquire/TUI flow) work
//!    without hanging on stdin forever.
//!
//! An optional hard `timeout` overlays both modes: whichever fires first
//! wins. Natural exit always beats both.

use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::Read;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// Configuration for one PTY capture session.
#[derive(Debug, Clone, Copy)]
pub struct CaptureConfig {
    /// Render after this much PTY silence (no bytes read) once the
    /// child has produced something. `Duration::ZERO` disables idle
    /// detection and reverts to the "wait for natural exit" behaviour.
    pub idle_after: Duration,
    /// Optional hard cap on real wall-clock time. When this fires we
    /// terminate the child and render whatever's buffered, even if the
    /// child is still actively writing.
    pub timeout: Option<Duration>,
}

impl CaptureConfig {
    /// Default behaviour for non-interactive renders: wait for natural
    /// exit, no idle detection, no hard cap.
    pub fn wait_for_exit() -> Self {
        Self {
            idle_after: Duration::ZERO,
            timeout: None,
        }
    }
}

/// How a capture loop terminated. Public so callers (and tests) can
/// distinguish a snapshot-on-idle render from a clean exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureReason {
    /// Child closed its PTY (EOF on the master read).
    Exited,
    /// `idle_after` elapsed since the last byte; the child was killed.
    Idle,
    /// `timeout` elapsed since capture started; the child was killed.
    Timeout,
}

/// Result of a PTY capture: the bytes we read and why we stopped.
#[derive(Debug, Clone)]
pub struct CaptureOutput {
    pub bytes: Vec<u8>,
    pub reason: CaptureReason,
}

/// Run `cmd` in a fresh PTY of the given size and return every byte the
/// child wrote to its slave end before the loop terminated.
///
/// See [`CaptureConfig`] for the exit conditions. The returned
/// [`CaptureOutput`] carries the [`CaptureReason`] so the caller can
/// surface "we snapshotted an interactive TUI" vs "the command exited"
/// to the operator.
///
/// # Errors
///
/// Returns `Err` when the PTY cannot be allocated, the command cannot
/// be spawned, or the child cannot be waited on after a clean exit.
pub fn run(cmd: &[String], cols: u16, rows: u16, config: CaptureConfig) -> Result<CaptureOutput> {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("opening pty")?;

    let mut command = build_command(cmd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("FORCE_COLOR", "1");
    command.env("CLICOLOR_FORCE", "1");

    let mut child = pty.slave.spawn_command(command).context("spawning child")?;
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().context("cloning reader")?;
    drop(pty.master);

    let mut killer = child.clone_killer();

    // Reader thread streams chunks through a channel so the main loop
    // can wait with timeouts. portable-pty's reader is blocking; the
    // only portable way to "read with timeout" is to push the read
    // onto its own thread and select on a channel here.
    let (tx, rx) = mpsc::channel::<ReadEvent>();
    let reader_handle = thread::spawn(move || reader_loop(reader, tx));

    let output = capture_loop(&rx, config, &mut *killer);

    // Drop the receiver: the reader thread's next `tx.send()` will
    // error out and it'll return. We do NOT `join()` because the
    // reader can be deep inside a blocking `read()` waiting on the
    // pipe to drain (e.g. `yes` floods the master end at MB/s). The
    // OS reaps it when our process exits or when the read finally
    // unblocks; either way it's not on the render critical path.
    drop(rx);
    drop(reader_handle);

    // Reap the child so we don't leave a zombie. After SIGKILL the
    // process is gone and this returns immediately. Use try_wait in a
    // short poll to defend against pathological cases (kill failed
    // silently, or natural-exit path where the kernel hasn't yet
    // delivered the status).
    let wait_deadline = Instant::now() + Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= wait_deadline => {
                // Last-resort SIGKILL: belt and braces.
                let _ = killer.kill();
                let _ = child.wait();
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => break,
        }
    }

    Ok(output)
}

/// What the reader thread sends back to the main loop.
enum ReadEvent {
    Bytes(Vec<u8>),
    Eof,
    /// I/O error on the master read. The error itself isn't surfaced
    /// to the operator (treated as a soft EOF): on macOS the kernel
    /// raises `EIO` when the slave closes, which is just "child
    /// exited" in disguise.
    ErrEof,
}

/// Blocking reader: forwards bytes to the main loop until EOF or error.
fn reader_loop(mut reader: Box<dyn Read + Send>, tx: mpsc::Sender<ReadEvent>) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                let _ = tx.send(ReadEvent::Eof);
                return;
            }
            Ok(n) => {
                if tx.send(ReadEvent::Bytes(buf[..n].to_vec())).is_err() {
                    return;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                let _ = tx.send(ReadEvent::ErrEof);
                return;
            }
        }
    }
}

/// Pure capture loop. Extracted so tests can drive it with a synthetic
/// channel and a no-op killer without standing up a real PTY.
fn capture_loop(
    rx: &mpsc::Receiver<ReadEvent>,
    config: CaptureConfig,
    killer: &mut dyn portable_pty::ChildKiller,
) -> CaptureOutput {
    // Some commands take 100ms+ to print the first line. If we honour
    // `idle_after` literally from t=0 with an empty buffer we'd fire
    // before the child ever spoke. Guard with `seen_any_byte` so idle
    // detection only arms after the child has written something.
    //
    // Edge case: the child writes nothing AND never exits AND no
    // timeout was set. We can't sit forever, so once
    // `idle_after * 5` has elapsed we kill anyway and report Idle on
    // an empty buffer.
    let mut buffer = Vec::new();
    let mut seen_any_byte = false;
    let mut last_byte_at = Instant::now();
    let start = Instant::now();

    let idle_armed = !config.idle_after.is_zero();
    // When the child is silent, force-fire after this many idle
    // periods even with an empty buffer. Documented above.
    const EMPTY_GRACE_MULTIPLIER: u32 = 5;

    loop {
        let now = Instant::now();

        // Hard timeout check before computing the next wait.
        if let Some(t) = config.timeout {
            if now.saturating_duration_since(start) >= t {
                // Drain queued bytes BEFORE killing. If we kill first,
                // the child's graceful-exit cleanup (e.g. a ratatui TUI
                // emitting `CSI 1049l` to leave the alternate screen)
                // ends up in our buffer and swaps the parser's active
                // grid back to the empty main screen, producing a blank
                // render.
                drain_remaining(rx, &mut buffer);
                let _ = killer.kill();
                return CaptureOutput {
                    bytes: buffer,
                    reason: CaptureReason::Timeout,
                };
            }
        }

        // How long until idle fires? Only meaningful once the child
        // has emitted at least one byte (or once the empty-grace cap
        // has elapsed).
        let idle_remaining = if idle_armed {
            let elapsed = now.saturating_duration_since(last_byte_at);
            let threshold = if seen_any_byte {
                config.idle_after
            } else {
                config.idle_after.saturating_mul(EMPTY_GRACE_MULTIPLIER)
            };
            threshold.saturating_sub(elapsed)
        } else {
            // Disabled: idle never fires.
            Duration::MAX
        };

        // How long until the hard timeout fires?
        let timeout_remaining = config
            .timeout
            .map(|t| t.saturating_sub(now.saturating_duration_since(start)))
            .unwrap_or(Duration::MAX);

        // Wait for the *earlier* of the two, or a chunk arriving.
        // Cap the recv_timeout at a reasonable upper bound so a stuck
        // child with no timeout configured can still be interrupted
        // by Ctrl-C (the channel disconnect path).
        let wait = idle_remaining
            .min(timeout_remaining)
            .min(Duration::from_secs(60));

        match rx.recv_timeout(wait) {
            Ok(ReadEvent::Bytes(chunk)) => {
                buffer.extend_from_slice(&chunk);
                seen_any_byte = true;
                last_byte_at = Instant::now();
            }
            Ok(ReadEvent::Eof) => {
                return CaptureOutput {
                    bytes: buffer,
                    reason: CaptureReason::Exited,
                };
            }
            Ok(ReadEvent::ErrEof) => {
                // I/O error on the master read. Treat as EOF: most
                // commonly this is "slave closed" (child exited) on
                // platforms where the OS surfaces that as an error
                // rather than read=0.
                return CaptureOutput {
                    bytes: buffer,
                    reason: CaptureReason::Exited,
                };
            }
            Err(RecvTimeoutError::Timeout) => {
                // Either idle fired, timeout fired, or our 60s safety
                // cap fired. The next loop iteration recomputes which
                // and acts.
                if idle_armed && idle_remaining == Duration::ZERO {
                    // Drain BEFORE killing. See the matching comment in
                    // the timeout branch above.
                    drain_remaining(rx, &mut buffer);
                    let _ = killer.kill();
                    return CaptureOutput {
                        bytes: buffer,
                        reason: CaptureReason::Idle,
                    };
                }
                // Otherwise continue: the timeout branch at the top
                // of the loop will pick up an expired deadline.
            }
            Err(RecvTimeoutError::Disconnected) => {
                // Reader thread is gone (panic or already returned).
                return CaptureOutput {
                    bytes: buffer,
                    reason: CaptureReason::Exited,
                };
            }
        }
    }
}

/// Pull any already-queued bytes out of the channel without blocking.
///
/// Called after we kill the child so we don't lose the last few chunks
/// the reader thread already pushed before we made the decision.
fn drain_remaining(rx: &mpsc::Receiver<ReadEvent>, buffer: &mut Vec<u8>) {
    while let Ok(ev) = rx.try_recv() {
        if let ReadEvent::Bytes(chunk) = ev {
            buffer.extend_from_slice(&chunk);
        }
    }
}

/// Build a `CommandBuilder` from the user's argv.
///
/// A single argv entry that contains whitespace is treated as a shell
/// command and dispatched through `$SHELL -c <cmd>` (falling back to
/// `/bin/sh`). Multi-argv invocations and single bare program names get
/// direct exec. This lets `cheese exec "isd ps"` work the same as
/// `cheese exec -- isd ps`, and supports pipes / redirects when quoted.
fn build_command(cmd: &[String]) -> CommandBuilder {
    if cmd.len() == 1 && cmd[0].contains(char::is_whitespace) {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut c = CommandBuilder::new(shell);
        c.args(["-c", &cmd[0]]);
        return c;
    }
    let mut c = CommandBuilder::new(&cmd[0]);
    if cmd.len() > 1 {
        c.args(&cmd[1..]);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test fake. portable-pty's ChildKiller is dyn-compatible, so we
    /// can hand the loop a counter-backed killer and assert on calls.
    #[derive(Debug)]
    struct FakeKiller {
        kills: Arc<AtomicUsize>,
    }

    impl portable_pty::ChildKiller for FakeKiller {
        fn kill(&mut self) -> std::io::Result<()> {
            self.kills.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
            Box::new(FakeKiller {
                kills: Arc::clone(&self.kills),
            })
        }
    }

    fn fake_killer() -> (FakeKiller, Arc<AtomicUsize>) {
        let kills = Arc::new(AtomicUsize::new(0));
        (
            FakeKiller {
                kills: Arc::clone(&kills),
            },
            kills,
        )
    }

    /// Natural exit: reader sends some bytes then EOF. Loop returns
    /// Exited and we never kill.
    #[test]
    fn natural_exit_returns_full_buffer() {
        let (tx, rx) = mpsc::channel();
        tx.send(ReadEvent::Bytes(b"hello ".to_vec())).unwrap();
        tx.send(ReadEvent::Bytes(b"world".to_vec())).unwrap();
        tx.send(ReadEvent::Eof).unwrap();

        let (mut killer, kills) = fake_killer();
        let out = capture_loop(
            &rx,
            CaptureConfig {
                idle_after: Duration::from_millis(50),
                timeout: None,
            },
            &mut killer,
        );

        assert_eq!(out.reason, CaptureReason::Exited);
        assert_eq!(out.bytes, b"hello world");
        assert_eq!(kills.load(Ordering::SeqCst), 0);
    }

    /// Idle path: bytes arrive, then silence, then the loop kills the
    /// child and returns Idle with the partial buffer. Crucially this
    /// is what makes interactive TUIs (fzf, inquire) snapshot instead
    /// of hanging forever.
    #[test]
    fn idle_terminates_after_silence_with_partial_buffer() {
        let (tx, rx) = mpsc::channel();
        let (mut killer, kills) = fake_killer();
        tx.send(ReadEvent::Bytes(b"prompt> ".to_vec())).unwrap();
        // Hold tx so the channel never disconnects: simulates a live
        // TUI sitting on stdin with nothing more to say.
        let _hold = tx;

        let start = Instant::now();
        let out = capture_loop(
            &rx,
            CaptureConfig {
                idle_after: Duration::from_millis(100),
                timeout: None,
            },
            &mut killer,
        );
        let elapsed = start.elapsed();

        assert_eq!(out.reason, CaptureReason::Idle);
        assert_eq!(out.bytes, b"prompt> ");
        assert_eq!(kills.load(Ordering::SeqCst), 1);
        // Sanity: we waited roughly idle_after, not 0 and not forever.
        assert!(
            elapsed >= Duration::from_millis(90),
            "fired too early: {:?}",
            elapsed
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "fired way too late: {:?}",
            elapsed
        );
    }

    /// Hard timeout: bytes keep arriving but the loop bails after the
    /// deadline anyway.
    #[test]
    fn timeout_terminates_even_under_constant_writes() {
        let (tx, rx) = mpsc::channel();
        let (mut killer, kills) = fake_killer();

        // Spam the channel every 20ms forever.
        let stop = Arc::new(AtomicUsize::new(0));
        let stop_for_thread = Arc::clone(&stop);
        let producer = thread::spawn(move || {
            while stop_for_thread.load(Ordering::SeqCst) == 0 {
                if tx.send(ReadEvent::Bytes(b"x".to_vec())).is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });

        let start = Instant::now();
        let out = capture_loop(
            &rx,
            CaptureConfig {
                idle_after: Duration::ZERO,
                timeout: Some(Duration::from_millis(150)),
            },
            &mut killer,
        );
        let elapsed = start.elapsed();

        stop.store(1, Ordering::SeqCst);
        let _ = producer.join();

        assert_eq!(out.reason, CaptureReason::Timeout);
        assert!(
            !out.bytes.is_empty(),
            "timeout should still surface buffered bytes"
        );
        assert_eq!(kills.load(Ordering::SeqCst), 1);
        assert!(
            elapsed >= Duration::from_millis(140),
            "fired too early: {:?}",
            elapsed
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "fired way too late: {:?}",
            elapsed
        );
    }

    /// Empty start: nothing arrives ever. Without the
    /// `EMPTY_GRACE_MULTIPLIER` cap we'd sit forever. With it, we
    /// terminate after `idle_after * 5` and return an empty buffer
    /// tagged Idle.
    #[test]
    fn empty_buffer_eventually_fires_idle() {
        let (tx, rx) = mpsc::channel::<ReadEvent>();
        let (mut killer, kills) = fake_killer();
        let _hold = tx;

        let start = Instant::now();
        let out = capture_loop(
            &rx,
            CaptureConfig {
                idle_after: Duration::from_millis(40),
                timeout: None,
            },
            &mut killer,
        );
        let elapsed = start.elapsed();

        assert_eq!(out.reason, CaptureReason::Idle);
        assert!(out.bytes.is_empty());
        assert_eq!(kills.load(Ordering::SeqCst), 1);
        // 5x the idle period, give or take scheduler jitter.
        assert!(
            elapsed >= Duration::from_millis(180),
            "fired before empty-grace expired: {:?}",
            elapsed
        );
    }

    /// Idle armed but child exits naturally before the silence
    /// threshold: reason must be Exited, not Idle. Natural exit
    /// always wins.
    #[test]
    fn natural_exit_beats_idle_when_idle_is_armed() {
        let (tx, rx) = mpsc::channel();
        let (mut killer, kills) = fake_killer();
        tx.send(ReadEvent::Bytes(b"done\n".to_vec())).unwrap();
        tx.send(ReadEvent::Eof).unwrap();
        let _hold = tx;

        let out = capture_loop(
            &rx,
            CaptureConfig {
                idle_after: Duration::from_secs(5),
                timeout: Some(Duration::from_secs(5)),
            },
            &mut killer,
        );

        assert_eq!(out.reason, CaptureReason::Exited);
        assert_eq!(out.bytes, b"done\n");
        assert_eq!(kills.load(Ordering::SeqCst), 0);
    }
}
