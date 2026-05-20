//! Pixel-equal golden regression suite for the cheese renderer.
//!
//! Each test feeds a canned ANSI byte stream through the deterministic
//! `render_canned_to_png` entry point and compares the resulting PNG
//! bytes against a committed golden under `tests/golden/`.
//!
//! Set `BLESS_GOLDEN=1` to overwrite the goldens (use after an
//! intentional render change). Without the env var, divergence fails
//! the test.

use cheese::{render_canned_to_png, test_render_opts};

// `isd ps`-style table: a header row, three node rows with colored
// status badges, and ASCII box-drawing borders. Status column uses
// red for "down" and green for "up". Approximates the v0.5 isd ps
// output shape; not a verbatim copy, just representative of the
// kind of table cheese will most often capture.
const ISD_PS_ANSI: &[u8] = concat!(
    "\x1b[1mNODE     STATUS   UPTIME   PORT\x1b[0m\r\n",
    "alpha    \x1b[32mup    \x1b[0m  12h      9001\r\n",
    "beta     \x1b[31mdown  \x1b[0m  ----     9002\r\n",
    "gamma    \x1b[32mup    \x1b[0m  3d       9003\r\n",
)
.as_bytes();

// `ls -la --color`-style output: a few rows with cyan directory
// names (dircolors default for `di`), white regular files, a green
// executable, and a blue symlink target indicator. Inline; we do
// not shell out to `ls` because the on-host `ls` is non-deterministic
// (mtimes, uid widths, group widths).
const LS_LA_ANSI: &[u8] = concat!(
    "total 24\r\n",
    "drwxr-xr-x  5 weaver staff  160 May 20 11:30 \x1b[36m.\x1b[0m\r\n",
    "drwxr-xr-x  3 weaver staff   96 May 20 11:00 \x1b[36m..\x1b[0m\r\n",
    "drwxr-xr-x  4 weaver staff  128 May 20 11:30 \x1b[36msrc\x1b[0m\r\n",
    "-rw-r--r--  1 weaver staff  504 May 20 11:30 Cargo.toml\r\n",
    "-rwxr-xr-x  1 weaver staff 1234 May 20 11:30 \x1b[32mbuild.sh\x1b[0m\r\n",
    "lrwxr-xr-x  1 weaver staff   12 May 20 11:30 \x1b[35mlink\x1b[0m -> Cargo.toml\r\n",
)
.as_bytes();

// Torture test: every 8 normal fg color, every 8 bg color, the three
// style flags we exercise via the golden (bold, underline,
// strikethrough), a 256-color sample, and a 24-bit RGB sample. Italic
// is intentionally omitted: the bundled FontSystem ships
// JetBrainsMono Regular only, and cosmic-text 0.12 panics with
// "no default font found" when shaping an italic span has zero face
// matches. Re-add the italic block once the renderer either loads an
// italic face or synthesises italic via skew. Each block resets back
// to default before the next so colors do not leak across cells.
const ANSI_RAINBOW_ANSI: &[u8] = concat!(
    // 8 fg colors.
    "fg: \x1b[30mK\x1b[31mR\x1b[32mG\x1b[33mY\x1b[34mB\x1b[35mM\x1b[36mC\x1b[37mW\x1b[0m\r\n",
    // 8 bg colors.
    "bg: \x1b[40m K \x1b[41m R \x1b[42m G \x1b[43m Y \x1b[44m B \x1b[45m M \x1b[46m C \x1b[47m W \x1b[0m\r\n",
    // Styles (italic excluded; see comment above).
    "style: \x1b[1mbold\x1b[0m \x1b[4munder\x1b[0m \x1b[9mstrike\x1b[0m\r\n",
    // 256-color sample: pick a few representative slots.
    "256: \x1b[38;5;196mR\x1b[38;5;46mG\x1b[38;5;21mB\x1b[38;5;226mY\x1b[38;5;201mM\x1b[0m\r\n",
    // 24-bit RGB sample.
    "rgb: \x1b[38;2;255;127;39mO\x1b[38;2;39;127;255mB\x1b[38;2;127;255;39mG\x1b[0m\r\n",
)
.as_bytes();

fn run_golden(bytes: &[u8], cols: usize, rows: usize, golden_path: &str) {
    let opts = test_render_opts();
    let actual = render_canned_to_png(bytes, cols, rows, &opts).expect("render must succeed");

    if std::env::var("BLESS_GOLDEN").is_ok() {
        std::fs::write(golden_path, &actual).expect("write golden");
        return;
    }

    let expected = std::fs::read(golden_path).unwrap_or_else(|_| {
        panic!("golden missing at {golden_path}: run with BLESS_GOLDEN=1 once to capture")
    });
    assert_eq!(
        actual.len(),
        expected.len(),
        "render diverged from golden at {golden_path}: size differs ({} vs {} bytes)",
        actual.len(),
        expected.len()
    );
    assert!(
        actual == expected,
        "render diverged from golden at {golden_path}: byte-equal compare failed"
    );
}

#[test]
fn isd_ps_renders_byte_identical() {
    run_golden(ISD_PS_ANSI, 80, 8, "tests/golden/isd-ps.png");
}

#[test]
fn ls_la_renders_byte_identical() {
    run_golden(LS_LA_ANSI, 80, 10, "tests/golden/ls-la.png");
}

#[test]
fn ansi_rainbow_renders_byte_identical() {
    run_golden(ANSI_RAINBOW_ANSI, 80, 8, "tests/golden/ansi-rainbow.png");
}
