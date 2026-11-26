# Win32 UI facts

Cached facts about the UI dependencies. Verify against the pinned versions only when a dependency changes.

## Text rendering

- `tiny-skia` 0.11 has no text API. Draw the panel with `tiny-skia` and draw text with GDI into a 32-bit DIB, then use the DIB's red channel as a coverage mask.
- GDI text APIs (`CreateFontW`, `DrawTextW`, `SetTextColor`, `SetBkMode`, `TRANSPARENT`, `DT_*`) are available under the already-enabled `Win32_Graphics_Gdi` feature.
- `Pixmap` data is premultiplied RGBA in byte order RGBA. Layered windows using `AC_SRC_ALPHA` expect premultiplied BGRA.

## Tray and notifications

- `tray-icon` 0.19 exposes `TrayIcon::set_tooltip(Option<S>)`; it truncates to 128 UTF-16 units.
- `tray-icon` has no balloon-notification API. Custom balloons require `Shell_NotifyIconW`, which needs the `Win32_UI_Shell` feature.
- `MessageBoxW` is available under the enabled `Win32_UI_WindowsAndMessaging` feature, with no console required.

## Monitors

- Use `MonitorFromWindow(None, MONITOR_DEFAULTTOPRIMARY)` for the primary monitor. A non-null window handle can resolve to that window's monitor instead.
- `GetMonitorInfoW` fills `MONITORINFO.rcWork` once `cbSize` is set. Use `rcWork` for a bottom-right position that clears the taskbar.
