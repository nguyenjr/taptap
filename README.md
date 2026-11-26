# TapTap

**TapTap** is a lightweight, high-performance animated mouse cursor overlay for Windows. Built in native Rust with the Win32 API, it replaces the system cursor with a spring-driven cartoon glove that reacts to rapid clicking with comic-book impact lines and synthesized sounds.

---

## Features

- **Transparent click-through overlay** — renders above applications without blocking the input underneath.
- **Spring-driven animation** — damped oscillators drive translation, wrist tilt, and scaling. Rapid clicks compound combo heat, winding the hand up for harder strikes.
- **Cartoon impact effects** — tapered comic rays, staggered burst timing, and secondary shockwave arcs around the fingertip.
- **Procedural audio** — five built-in click sounds (Wood Knock, Click, Bell, balloon-pop Pop, and Jingle Bell) generated in memory with no asset files. Combo heat raises pitch and intensity; volume runs from Off to 100%.
- **Presentation Mode** — while the overlay is Active, suppresses mouse clicks globally while keeping the animation and sound, so you can point at screen elements without activating them. A status line at the bottom-right of the screen shows while Presentation Mode is suppressing clicks and names the exit hotkey.
- **Customizable hand size** — presets (75%, 100%, 135%, 175%) plus 10% steps, clamped to 50–200%.
- **Hand styles** — Classic and Gingerbread, with the same geometry and hotspot; the Gingerbread Hand is light brown with white icing piping and a squiggle cuff.
- **Robust failsafes** — restores the system cursors on exit, crash, or emergency termination.
- **Single-instance enforcement** — a global OS mutex guarantees only one instance runs.
- **Per-monitor DPI scaling** — keeps the hand correctly sized on mixed-DPI displays.
- **Persisted settings** — hand size, click sound, and volume survive restarts.

---

## Global Hotkeys

| Shortcut | Action |
| --- | --- |
| `Ctrl + Shift + P` | Toggle the overlay Active / Inactive (restores the system cursor) |
| `Ctrl + Shift + M` | Toggle Presentation Mode (blocks clicks while keeping animation and sound) |
| `Ctrl + Shift + =` | Increase hand size by 10% |
| `Ctrl + Shift + -` | Decrease hand size by 10% |
| `Ctrl + Shift + Q` | Emergency quit and cursor restore |

---

## System Tray

Right-click the tray icon to:

- Toggle Active and Presentation Mode.
- Choose a hand size from the presets or the 10% increase/decrease items.
- Choose a Hand Style: Classic or Gingerbread.
- Choose a Click Sound: Wood Knock, Click, Bell, Pop, or Jingle Bell.
- Choose a Volume: Off, Quiet (25%), 50%, 75%, or 100%. `Off` replaces the old Mute Audio item.
- Quit.

While the overlay is Active, Presentation Mode deliberately suppresses mouse clicks across the desktop, so the tray cannot be used while it is on — use the hotkeys to leave that state or quit. Toggling the overlay Inactive ends click suppression.

---

## Settings

Hand style, hand size, click sound, and volume are saved to `%APPDATA%\taptap\settings.txt` (or `settings.txt` in the current directory if `APPDATA` is unset). The file uses simple `key=value` lines:

```text
scale_pct=100
hand_style=classic
sound=wood
volume=100
```

Missing or invalid values fall back to the defaults (`100`, `classic`, `wood`, `100`); unknown keys are ignored. `hand_style` accepts `classic` or `gingerbread`; `sound` also accepts `jingle`. Set `volume=0` (or pick **Off** in the tray) for silence.

---

## Building from Source

### Prerequisites

- Rust toolchain via [rustup](https://rustup.rs/); the version is pinned in `rust-toolchain.toml` (1.100.0) and installed automatically
- MSVC toolchain on Windows (`rc.exe` is needed by `build.rs` for the embedded icon)
- Cargo (bundled with Rust)

### Compilation Steps

1. Clone the repository:

```bash
git clone https://github.com/nguyenjr/taptap.git
cd taptap
```

2. Build the release binary (size optimizations and embedded Windows resources/icon):

```bash
cargo build --release
```

3. Find the standalone executable at:

```text
target\release\taptap.exe
```

Release builds run without a console window.

---

## Releasing

Releases are published automatically when a version tag is pushed:

```bash
git tag -a v0.1.0 -m "Release 0.1.0"
git push origin v0.1.0
```

The `release` workflow builds `taptap.exe` and attaches it to a GitHub Release with generated notes. Tag only commits that have passed CI on `main`.
