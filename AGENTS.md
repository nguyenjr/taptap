# taptap

Windows-only Rust app: Win32 overlay driven by the `windows` 0.58 crate. Requires Windows + MSVC toolchain pinned in `rust-toolchain.toml` (`winresource` in `build.rs` needs `rc.exe`). No rustfmt/clippy config, no `tests/`.

## Commands

- One-time setup: `git config core.hooksPath .githooks` enables `.githooks/pre-push`, which rejects direct pushes to `main`.
- `cargo build --release` — shippable binary at `target\release\taptap.exe`. Release enables `windows_subsystem = "windows"` (no console), `opt-level="z"`, LTO, strip.
- `cargo run` / `cargo build` — debug keeps a console. Errors are reported via `eprintln!` (`Effect::ReportError`), so use a debug build to see startup failures.
- `cargo test` — all tests are inline `#[cfg(test)] mod tests` in `src/*.rs`; there are no integration tests.
  - `cargo test app::tests` — pure state-machine tests via `FakePlatform`; no GUI needed.
  - `cargo test settings::tests` — temp-dir files, safe.
  - `cargo test menu_quit_event_is_delivered_on_the_first_message` (`main.rs`) — real tray/message-loop test; needs a desktop session and posts `WM_QUIT` after 300 ms, so it can hang headless.
- `cargo fmt` / `cargo clippy` use tool defaults (no config files); `.github/workflows/checks.yml` runs `fmt --check`, `clippy -D warnings`, tests minus the desktop-only test, a release build, `cargo audit`, and the desktop test in a separate 5-minute job.
- Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds and publishes `taptap.exe` as a GitHub Release.

## Architecture

- `src/main.rs` — entrypoint and all Win32: layered window, low-level mouse hook, tray menu, render thread, message loop, single-instance mutex `Local\TapTap_SingleInstance_Mutex` (second launch exits silently).
- `src/overlay_state.rs` — Overlay State is the single authority for Active, Presentation Mode, Hand Style, Hand Size, Click Sound and Volume; `snapshot()` / `apply(Command)` are its interface, with atomics hidden as implementation detail.
- `src/platform.rs` — `Platform` trait and `Event`/`Effect` enums are the seam; test-only `FakePlatform` lets logic be tested without Windows. State changes cross it as one `StateChanged` effect, not per-value writes.
- `src/app.rs` — maps events to commands and emits side effects; holds no state of its own. Put event routing here, state rules in `overlay_state.rs`, Win32 fan-out in `main.rs`.
- Hotkeys: `HOTKEY_BINDINGS` in `main.rs` holds IDs 1–5, `Ctrl+Shift`, `MOD_NOREPEAT` and raw VK codes. A new/changed hotkey means editing that table, `Hotkey` in `platform.rs`, its `app.rs` mapping, and the README table.
- Tray menu: `menu_bindings()` in `main.rs` is the single source for item IDs, labels and `MenuAction`s (hand rows come from `HAND_STYLES`); `menu_action` is a table lookup with a direct unit test. Add menu items there.
- `src/hand_art.rs` is shared SVG markup: `#[path]`-included by `build.rs` (generates the .ico), rendered for the runtime hand (`physics_render.rs`) and tray icon (`main.rs`). Edit it and all three change; no image assets. `HAND_STYLES` is the append-only registry (id, menu id, label, markup, silhouette, arm spec) for Classic/Gingerbread and future Hands — add a row, never reorder.
- `src/audio.rs` synthesizes all five WAVs in memory at startup (rodio, `wav` feature only); no audio assets.
- `src/cursor.rs` swaps all 13 system cursors for a blank cursor and restores them on shutdown, panic, Ctrl-C and Ctrl-close — don't leave cursors hidden while debugging.
- Settings: `%APPDATA%\taptap\settings.txt` (or `settings.txt` in cwd when `APPDATA` unset), `key=value` for `scale_pct`/`hand_style`/`sound`/`volume`; saved via temp file + rename. Scale clamps to 50–200, volume to 0–100.

## Agent workflows

- Never push directly to `main`: work on a feature branch, push that branch, and merge it into `main`.
- Issues/specs live under `.scratch/<feature-slug>/`: `spec.md` plus one `issues/NN-slug.md` per ticket, numbered from 01. `.scratch/` is gitignored — these files are local-only. See `docs/agents/issue-tracker.md`.
- Triage `Status:` roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.
- Before domain work, read `GLOSSARY.md`, `GLOSSARY-MAP.md` (if present) and relevant `docs/adr/`; if absent, proceed silently. See `docs/agents/domain.md`.
