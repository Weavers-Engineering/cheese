# cheese

Say cheese. Get a pixel-perfect screenshot of your terminal.

```sh
cheese exec "isd ps"   # v0.1: runs the command in a real PTY, renders to PNG
cheese capture          # v0.2: screenshots your running terminal pane
```

## Why

Existing tools (freeze, t-rec, asciinema) all run the command in a constrained pty of their own, then re-render with their font and their theme. The output is a *similar-looking* render, not your terminal.

`cheese` reads your terminal's font and theme (iTerm2 plist, Ghostty / Alacritty / Kitty config), captures via a real PTY, and renders with your actual styling. The PNG looks like what you'd see in your terminal because it is what you'd see in your terminal.

## Status

v0.0.1 ships the CLI chassis only. Subsequent releases:

- v0.1: `cheese exec <cmd>` (PTY capture + vt100 parsing + tiny-skia render).
- v0.2: theme + font auto-detection across iTerm2 / Ghostty / Alacritty / Kitty.
- v0.3: `cheese capture` (live-pane screenshot via terminal-RPC).

## Install

```sh
cargo install cheese
```

(Once v0.1 lands.)

## License

MIT.
