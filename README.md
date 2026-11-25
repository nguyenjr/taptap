# taptap

A Windows cursor overlay that replaces the system pointer with an animated cartoon hand. Clicks poke and tilt the hand, fire a comic impact burst, and play a short synthesized sound. Rapid clicks build "combo heat", which escalates the animation and the pitch of the sound.

## Controls

### Keyboard

| Shortcut | Action |
| --- | --- |
| `Ctrl+Shift+P` | Toggle the overlay on/off |
| `Ctrl+Shift+M` | Toggle Presentation Mode |
| `Ctrl+Shift+=` | Increase hand size by 10% |
| `Ctrl+Shift+-` | Decrease hand size by 10% |
| `Ctrl+Shift+Q` | Quit |

### Tray menu

- **Active** / **Presentation Mode** — the same toggles as the hotkeys.
- **Hand Size** — presets (75%, 100%, 135%, 175%) plus 10% increase/decrease. Values are clamped to 50–200%.
- **Click Sound** — Wood Knock, Click, Bell, or Pop.
- **Volume** — Off, Quiet (25%), 50%, 75%, or 100%.
- **Quit**.

**Presentation Mode deliberately suppresses mouse clicks across the desktop.** The tray menu cannot be clicked while it is on, so use the keyboard hotkeys to leave that state or quit.

## Settings

Hand size, click sound, and volume are saved to `%APPDATA%\taptap\settings.txt` (or `settings.txt` in the current directory if `APPDATA` is unset). The file uses simple `key=value` lines:

```text
scale_pct=100
sound=wood
volume=100
```

Missing or invalid values fall back to the defaults (`100`, `wood`, `100`); unknown keys are ignored. Set `volume=0` (or pick **Off** in the tray) for silence.

## Notes

- Windows only; build with `cargo build --release`. Release builds run without a console window.
- Single instance: launching again exits without disturbing the running one.
- The system cursor is restored on quit, on Ctrl+C / window close, and on panic.
- Per-monitor DPI scaling keeps the hand sized correctly on mixed-DPI displays.
- Artwork and sounds are generated at runtime; there are no external asset files.
