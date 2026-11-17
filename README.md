# TapTap 🐾🖱️

**TapTap** is a lightweight, high-performance animated mouse cursor overlay for Windows. Built in native Rust and leveraging the Win32 API, it replaces your system cursor with a snappy, spring physics-driven cartoon glove that reacts dynamically to rapid clicking with comic-book impact lines, procedural wood-knock sound effects, and zero input lag.

---

## ✨ Features

* **Transparent Click-Through Overlay:** Renders smoothly above all standard applications, games, and right-click context menus without blocking mouse interactions underneath.
* **Spring-Driven Animation & Physics:** Uses damped harmonic oscillators to calculate real-time translation, wrist-tilt rotation, and scaling. Rapid clicking compounds combo heat, causing the hand to wind up and strike harder.
* **Cartoon Impact Effects:** Spawns tapered comic teardrop rays, staggered burst timing, and secondary shockwave arcs around the fingertip on every click.
* **Procedural Audio:** Synthesizes a crisp, pitch-shifted wood-knock sound effect in memory on every click with zero disk latency.
* **Presentation Mode:** Toggles click-suppression so you can visually tap on screen elements without triggering underlying links, buttons, or advancing slides.
* **Customizable Hand Size:** Easily switch between `Small (75%)`, `Medium (100%)`, `Large (135%)`, and `Extra Large (175%)` via the system tray or keyboard shortcuts.
* **Robust Failsafes:** Automatically restores default system cursors on exit, crash, or emergency termination.
* **Single-Instance Enforcement:** Uses a global OS kernel mutex to guarantee only one instance runs at a time.

---

## ⌨️ Global Hotkeys

| Shortcut | Action |
| --- | --- |
| `Ctrl + Shift + P` | Toggle TapTap Overlay Active / Inactive (Restores system cursor) |
| `Ctrl + Shift + M` | Toggle Presentation Mode (Blocks clicks while keeping animations/sound) |
| `Ctrl + Shift + =` | Increase Hand Size (+10%) |
| `Ctrl + Shift + -` | Decrease Hand Size (-10%) |
| `Ctrl + Shift + Q` | Emergency Quit & Restore Cursors |

---

## 🚀 Building from Source

### Prerequisites

* **Rust toolchain** (Stable MSVC toolchain via [rustup](https://rustup.rs/))
* **Cargo** (comes bundled with Rust)

### Compilation Steps

1. Clone the repository:
```bash
git clone https://github.com/your-username/taptap.git
cd taptap

```


2. Build the release binary (with size optimizations and embedded Windows resources/icon):
```bash
cargo build --release

```


3. Find your standalone executable at:
```text
target\release\taptap.exe

```



---

## 🎛️ System Tray Integration

Once launched, TapTap runs silently in the Windows system tray (near the system clock). Right-clicking the gold tray icon lets you:

* Toggle Active state and Presentation Mode
* Mute or unfreeze audio effects
* Choose preset hand sizes
* Quit the application
