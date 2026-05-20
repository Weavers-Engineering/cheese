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
