//! End-to-end smoke test for the `cheese` binary.
//!
//! Runs `cheese exec echo hello` against a freshly built binary and
//! checks that the output file exists and starts with the 8-byte PNG
//! magic header. NOT a pixel-equal test: PTY timing makes the
//! captured grid non-deterministic on different hosts and even
//! across runs on the same host. Pixel-equal coverage lives in
//! `tests/golden_render.rs`.

use assert_cmd::Command;
use std::fs;
use std::time::{Duration, Instant};

const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

#[test]
fn exec_echo_writes_valid_png() {
    let tmp = tempdir();
    let out_path = tmp.join("smoke.png");

    Command::cargo_bin("cheese")
        .expect("binary built")
        .args([
            "exec",
            "--output",
            out_path.to_str().expect("utf8 path"),
            "echo",
            "hello",
        ])
        .assert()
        .success();

    let bytes = fs::read(&out_path).expect("output png exists");
    assert!(bytes.len() >= 8, "png too short: {} bytes", bytes.len());
    assert_eq!(
        &bytes[..8],
        &PNG_MAGIC,
        "output does not start with PNG magic header"
    );

    // Clean up: the tempdir helper does not auto-delete (we keep
    // assert_cmd as the only dev dep, no `tempfile`), so we tidy
    // up on success. On failure, leaving the file behind helps
    // post-mortem debugging.
    let _ = fs::remove_file(&out_path);
    let _ = fs::remove_dir(&tmp);
}

/// `--idle 200ms` must cut a "print then hang" command short. Before
/// this feature the loop blocked on `child.wait()` and the sleep
/// would run to completion. Shell prints `READY` then sleeps 5s; with
/// idle detection we should snapshot ~200ms after READY lands.
#[test]
fn idle_flag_snapshots_long_running_command() {
    let tmp = tempdir();
    let out_path = tmp.join("idle.png");

    let start = Instant::now();
    Command::cargo_bin("cheese")
        .expect("binary built")
        .args([
            "--idle",
            "200ms",
            "--output",
            out_path.to_str().expect("utf8 path"),
            "sh",
            "-c",
            "printf READY; sleep 5",
        ])
        .assert()
        .success();
    let elapsed = start.elapsed();

    // Budget loose enough to absorb parallel-test contention on CI
    // while still being far below the 5-second sleep target. Before
    // the idle path existed this would have taken a full 5s.
    assert!(
        elapsed < Duration::from_secs(4),
        "cheese took {:?} for printf+sleep with --idle 200ms (should snapshot quickly)",
        elapsed
    );

    let bytes = fs::read(&out_path).expect("output png exists");
    assert_eq!(&bytes[..8], &PNG_MAGIC);

    let _ = fs::remove_file(&out_path);
    let _ = fs::remove_dir(&tmp);
}

/// `--timeout 300ms` must cap a steadily-writing long runner. The
/// shell loop ticks once per 50ms forever; idle detection is disabled
/// (`--idle 0`) so only the timeout can stop us. Budget is loose to
/// absorb debug-build render time on CI.
#[test]
fn timeout_flag_caps_runaway_command() {
    let tmp = tempdir();
    let out_path = tmp.join("timeout.png");

    let start = Instant::now();
    Command::cargo_bin("cheese")
        .expect("binary built")
        .args([
            "--idle",
            "0",
            "--timeout",
            "300ms",
            "--output",
            out_path.to_str().expect("utf8 path"),
            "sh",
            "-c",
            "while :; do printf tick; sleep 0.05; done",
        ])
        .assert()
        .success();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "cheese took {:?} for tick-loop with --timeout 300ms",
        elapsed
    );

    let bytes = fs::read(&out_path).expect("output png exists");
    assert_eq!(&bytes[..8], &PNG_MAGIC);

    let _ = fs::remove_file(&out_path);
    let _ = fs::remove_dir(&tmp);
}

fn tempdir() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("cheese-smoke-{pid}-{nanos}"));
    fs::create_dir_all(&p).expect("create tempdir");
    p
}
