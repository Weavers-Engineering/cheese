# cheese

Say cheese. Get a pixel-perfect screenshot of your terminal.

```sh
cheese isd ps                # default: spawn in a PTY, copy PNG to clipboard
cheese -o out.png ls -la     # write a file instead of clipboard
cat log.txt | cheese         # pipe raw bytes through (lossy: source sees no tty)
cheese exec ls               # explicit exec form, identical to the default
cheese capture               # v0.2: screenshot your running terminal pane
```

**Two modes:**

- **`cheese <cmd>...`** spawns the command in a real PTY. The source sees `isatty(stdout) = true`, emits full ANSI colors, and gets your real terminal width. This is canonical: pass your command and get a render that matches what you'd see in the terminal.
- **`... | cheese`** renders the raw piped bytes verbatim. Useful for log files, here-docs, programmatic feeds. Lossy when the source tool strips ANSI / collapses width on pipe (which is most modern CLIs). For tty-sensitive tools, use `cheese <cmd>` instead.

## Why

Existing tools (freeze, t-rec, asciinema) run the command in a constrained pty of their own, then re-render with their font and their theme. The output is a *similar-looking* render, not your terminal.

`cheese` reads your terminal's font and theme (iTerm2 plist, Ghostty / Alacritty / Kitty config), captures via a real PTY, and renders with your actual styling. The PNG looks like what you'd see in your terminal because it is what you'd see in your terminal.

## Status

v0.0.x: working toward v0.1.

- [x] **Phase 1**: CLI chassis (clap, version, exit shape).
- [x] **Phase 2**: PTY capture (`portable-pty`) + VT state parse (`alacritty_terminal`).
- [x] **Phase 3**: Renderer (`tiny-skia` + `cosmic-text`), Tokyo Night theme, JetBrains Mono Nerd Font.
- [ ] **Phase 4**: Golden-PNG regression tests.
- [ ] **Phase 5**: `--chrome=mac`, padding, cursor polish.
- [ ] **Phase 6**: `cargo-dist` prebuilt binaries (v0.1.0 release).

After v0.1:

- **v0.2**: theme + font auto-detection across iTerm2 / Ghostty / Alacritty / Kitty.
- **v0.3**: `cheese capture` (live-pane screenshot via terminal-RPC).

## Install

```sh
cargo install cheese
```

Pure Rust, no external toolchain. Once `cargo-dist` lands (Phase 6), prebuilt darwin-arm64, darwin-x86_64, linux-x86_64, and linux-aarch64 binaries will also ship on GitHub Releases.

## License

[MIT](LICENSE). Bundles JetBrains Mono Nerd Font Regular under the SIL Open Font License 1.1.
