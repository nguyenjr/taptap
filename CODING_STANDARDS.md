# Coding Standards

Read during review. These are judgement calls; mechanical rules belong in `cargo fmt`, `cargo clippy`, or CI.

## Layers

- `src/app.rs` maps events to commands and emits effects. It holds no state of its own; event routing stays here.
- State rules and per-session bookkeeping live in `src/overlay_state.rs`.
- Win32 fan-out lives in `src/main.rs`.

## Language

- Name domain concepts with the exact terms in `GLOSSARY.md`, in code and in user-visible text.
- Derive displayed shortcut text from `HOTKEY_BINDINGS`; never repeat a hotkey label in a string literal.

## Tests

- Assert observable behaviour at the pre-agreed seams: emitted effects via `FakePlatform`, pure functions for user-visible strings and pixel blending.
- Keep GUI rendering and placement verification manual; do not add desktop-dependent assertions to CI.
