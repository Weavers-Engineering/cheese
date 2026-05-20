# cheese

Say cheese. Get a pixel-perfect screenshot of your terminal.

```sh
cheese exec "isd ps"   # v0.1: run the command in a real PTY, render to PNG
cheese capture          # v0.2: screenshot your running terminal pane
```

## Why

Existing tools (freeze, t-rec, asciinema) run the command in a constrained pty of their own, then re-render with their font and their theme. The output is a *similar-looking* render, not your terminal.

`cheese` reads your terminal's font and theme (iTerm2 plist, Ghostty / Alacritty / Kitty config), captures via a real PTY, and renders with your actual styling. The PNG looks like what you'd see in your terminal because it is what you'd see in your terminal.

## Status

v0.0.x: working toward v0.1.

- [x] **Phase 1**: CLI chassis (clap, version, exit shape).
- [x] **Phase 2**: PTY capture (`portable-pty`) + VT state parse (`libghostty-vt`).
- [x] **Phase 3**: Renderer (`tiny-skia` + `cosmic-text`), Tokyo Night theme, JetBrains Mono Nerd Font.
- [ ] **Phase 4**: Golden-PNG regression tests.
- [ ] **Phase 5**: `--chrome=mac`, padding, cursor polish.
- [ ] **Phase 6**: `cargo-dist` prebuilt binaries (v0.1.0 release).

After v0.1:

- **v0.2**: theme + font auto-detection across iTerm2 / Ghostty / Alacritty / Kitty.
- **v0.3**: `cheese capture` (live-pane screenshot via terminal-RPC).

## Install

### Prebuilt binary (recommended, post v0.1.0)

GitHub Releases will ship self-contained darwin-arm64, darwin-x86_64, linux-x86_64, and linux-aarch64 binaries via `cargo-dist`. No toolchain required at install time. Available after the v0.1.0 tag lands.

### From source

```sh
cargo install cheese
```

Requires **Zig 0.15.x** on the build host. `libghostty-vt-sys` (the Ghostty terminal core wrapper) builds its C surface through Ghostty's `build.zig`, which hard-fails on Zig 0.16+. Install with `brew install zig@0.15` on macOS or grab the 0.15.x tarball from `https://ziglang.org/download/`.

Once a prebuilt binary install path exists (Phase 6), the Zig requirement only applies if you build from source.

## License

[MIT](LICENSE). Bundles JetBrains Mono Nerd Font Regular under the SIL Open Font License 1.1.
