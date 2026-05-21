# Changelog

All notable changes to `cheese` land here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning is [SemVer](https://semver.org/spec/v2.0.0.html).

## [0.1.0] : 2026-05-21

First tagged release. Pure-Rust, no external toolchain. Prebuilt binaries for darwin-arm64, darwin-x86_64, linux-x86_64, linux-aarch64 ship on GitHub Releases.

### Features

- PTY capture via `portable-pty` and VT parse via `alacritty_terminal` (pure Rust, no Zig/libghostty dependency). (#1, #4)
- Renderer: `tiny-skia` + `cosmic-text` with Tokyo Night theme and JetBrains Mono Nerd Font Regular bundled. (#2)
- Golden-PNG regression suite covering the renderer surface. (#3)
- `cheese exec` form: synthetic shell prompt, auto-fit height, clipboard-by-default output, single-arg shell-out so `cheese exec "isd ps"` works. (#5, #6)
- Auto-detect terminal cols and rows from the host pty (no manual `--cols` / `--rows` flags required). (#7)
- Pipe mode: `... | cheese` reads stdin and renders it; the sibling command is captured at startup via a `ps` shellout and re-execed in a PTY so the source still sees `isatty(stdout) = true`. (#8, #9, #10, #11, #12)
- `cheese <cmd>` is the default exec form. `cheese exec <cmd>` stays as the explicit alias. (#13)
- `--chrome=mac` window chrome with cursor and padding wiring (Phase 5). (#16)
- `cargo-dist` packaging: shell installer, source tarball, per-target xz archives, SHA256 checksums, plan-mode CI on PRs.

### Fixes

- Render italic spans by synthesizing a 12 degree skew transform when the font has no italic face (avoids cosmic-text fallback churn). (#15)
- Pipe sibling scan skips our own `ps` shellout so `cheese`'s pipe mode no longer captures itself as the sibling. (#12)
- Revert pipe-rerun magic that ran the captured sibling a second time. The default is now `cheese <cmd>` for explicit re-exec. (#13)

### Build / CI

- GitHub Actions CI workflow: fmt, clippy `-D warnings`, `cargo test --release`. (#14)
- `cargo-dist` GitHub Actions release workflow at `.github/workflows/release.yml`, triggered by `v*.*.*` tags. Builds four targets and uploads to GitHub Releases.

### Docs

- README install section points at the cargo-dist shell installer; source build via `cargo install --git` documented as the dev fallback.
- Phase 4 / 5 / 6 status ticks in the README status table.

[0.1.0]: https://github.com/Weavers-Engineering/cheese/releases/tag/v0.1.0
