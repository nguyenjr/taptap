#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod cursor;
mod hand_art;
mod physics_render;

use audio::AudioEngine;
use crossbeam_channel::{unbounded, Sender};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use physics_render::{PokeAnimator, CANVAS_SIZE, HOTSPOT_X, HOTSPOT_Y};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tray_icon::{Icon, TrayIconBuilder};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, COLORREF, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM,
    LRESULT, POINT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, MonitorFromPoint,
    ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER,
    BLENDFUNCTION, DIB_RGB_COLORS, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_OEM_MINUS, VK_OEM_PLUS,
};
use windows::Win32::UI::WindowsAndMessaging::*;

static ACTIVE: AtomicBool = AtomicBool::new(true);
static PRESENTATION_MODE: AtomicBool = AtomicBool::new(false);
static MUTED: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(true);

static USER_SCALE_PCT: AtomicU32 = AtomicU32::new(100);

static MOUSE_X: AtomicI32 = AtomicI32::new(500);
static MOUSE_Y: AtomicI32 = AtomicI32::new(500);

#[derive(Clone, Copy)]
enum InputEvent {
    LeftDown,
    LeftUp,
    RightEvent,
}

#[derive(Clone, Copy)]
enum Command {
    ToggleActive,
    TogglePresentation,
    ToggleMute,
    ScaleUp,
    ScaleDown,
    SetScalePreset(u32),
    Quit,
}

static EVENT_TX: OnceLock<Sender<InputEvent>> = OnceLock::new();

const HOTKEY_TOGGLE_ACTIVE: i32 = 1;
const HOTKEY_TOGGLE_PRESENTATION: i32 = 2;
const HOTKEY_QUIT: i32 = 3;
const HOTKEY_SIZE_UP: i32 = 4;
const HOTKEY_SIZE_DOWN: i32 = 5;

const SIZE_PRESETS: [(u32, &str); 4] = [
    (75, "Small (75%)"),
    (100, "Medium (100% - Default)"),
    (135, "Large (135%)"),
    (175, "Extra Large (175%)"),
];

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let hook_struct = *(lparam.0 as *const MSLLHOOKSTRUCT);
        MOUSE_X.store(hook_struct.pt.x, Ordering::Relaxed);
        MOUSE_Y.store(hook_struct.pt.y, Ordering::Relaxed);

        let msg = wparam.0 as u32;
        if ACTIVE.load(Ordering::Relaxed) {
            if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONDBLCLK {
                if let Some(tx) = EVENT_TX.get() {
                    let _ = tx.try_send(InputEvent::LeftDown);
                }
                if PRESENTATION_MODE.load(Ordering::Relaxed) {
                    return LRESULT(1);
                }
            } else if msg == WM_LBUTTONUP {
                if let Some(tx) = EVENT_TX.get() {
                    let _ = tx.try_send(InputEvent::LeftUp);
                }
                if PRESENTATION_MODE.load(Ordering::Relaxed) {
                    return LRESULT(1);
                }
            } else if msg == WM_RBUTTONDOWN || msg == WM_RBUTTONUP || msg == WM_RBUTTONDBLCLK {
                if let Some(tx) = EVENT_TX.get() {
                    let _ = tx.try_send(InputEvent::RightEvent);
                }
                if PRESENTATION_MODE.load(Ordering::Relaxed) {
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn query_monitor_dpi_scale(x: i32, y: i32) -> f32 {
    unsafe {
        let hmon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        if GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
            (dpi_x as f32 / 96.0).clamp(1.0, 2.25)
        } else {
            1.0
        }
    }
}

fn spawn_render_thread(hwnd_raw: isize, rx: crossbeam_channel::Receiver<InputEvent>) {
    std::thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut _);
        let mut animator = PokeAnimator::new();
        let audio = AudioEngine::new();

        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: CANVAS_SIZE as i32,
                    biHeight: -(CANVAS_SIZE as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbitmap = CreateDIBSection(
                mem_dc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                HANDLE::default(),
                0,
            )
            .expect("Failed to allocate DIBSection");
            let old_obj = SelectObject(mem_dc, hbitmap);

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let mut last_tick = Instant::now();
            let mut was_visible = true;
            let mut frame_count: u32 = 0;

            while RUNNING.load(Ordering::Relaxed) {
                let now = Instant::now();
                let dt = (now - last_tick).as_secs_f32().min(0.05);
                last_tick = now;

                let mut force_topmost = false;

                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        InputEvent::LeftDown => {
                            animator.trigger_click(true);
                            if !MUTED.load(Ordering::Relaxed) {
                                audio.play_poke(animator.combo_heat);
                            }
                            force_topmost = true;
                        }
                        InputEvent::LeftUp => {
                            animator.trigger_click(false);
                            force_topmost = true;
                        }
                        InputEvent::RightEvent => {
                            force_topmost = true;
                        }
                    }
                }

                if !ACTIVE.load(Ordering::Relaxed) {
                    if was_visible {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                        was_visible = false;
                    }
                    std::thread::sleep(Duration::from_millis(16));
                    continue;
                } else if !was_visible {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    was_visible = true;
                    force_topmost = true;
                }

                frame_count = frame_count.wrapping_add(1);
                if force_topmost || frame_count % 10 == 0 {
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOSENDCHANGING,
                    );
                }

                let mx = MOUSE_X.load(Ordering::Relaxed);
                let my = MOUSE_Y.load(Ordering::Relaxed);
                let dpi_scale = query_monitor_dpi_scale(mx, my);
                let user_scale = USER_SCALE_PCT.load(Ordering::Relaxed) as f32 / 100.0;
                let pres_mode = PRESENTATION_MODE.load(Ordering::Relaxed);

                animator.step(dt, user_scale);
                let pixels = animator.render_bgra(dpi_scale, user_scale, pres_mode);

                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr(),
                    bits_ptr as *mut u8,
                    pixels.len(),
                );

                let dst_pt = POINT {
                    x: mx - HOTSPOT_X,
                    y: my - HOTSPOT_Y,
                };
                let wnd_size = SIZE {
                    cx: CANVAS_SIZE as i32,
                    cy: CANVAS_SIZE as i32,
                };
                let src_pt = POINT { x: 0, y: 0 };

                let _ = UpdateLayeredWindow(
                    hwnd,
                    screen_dc,
                    Some(&dst_pt),
                    Some(&wnd_size),
                    mem_dc,
                    Some(&src_pt),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );

                std::thread::sleep(Duration::from_millis(8));
            }

            SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    });
}

struct TrayControls {
    _tray: tray_icon::TrayIcon,
    item_active: CheckMenuItem,
    item_pres: CheckMenuItem,
    item_mute: CheckMenuItem,
    size_items: Vec<(u32, CheckMenuItem)>,
    item_size_up: MenuItem,
    item_size_down: MenuItem,
    item_quit: MenuItem,
}

impl TrayControls {
    fn sync_size_checks(&self, active_pct: u32) {
        for (pct, item) in &self.size_items {
            item.set_checked(*pct == active_pct);
        }
    }
}

fn create_tray_icon() -> TrayControls {
    let menu = Menu::new();
    let item_active = CheckMenuItem::new("Active (Ctrl+Shift+P)", true, true, None);
    let item_pres = CheckMenuItem::new("Presentation Mode (Ctrl+Shift+M)", true, false, None);
    let item_mute = CheckMenuItem::new("Mute Audio", true, false, None);

    let size_submenu = Submenu::new("Hand Size", true);
    let mut size_items = Vec::with_capacity(SIZE_PRESETS.len());
    for (pct, label) in SIZE_PRESETS {
        let is_default = pct == 100;
        let check_item = CheckMenuItem::new(label, true, is_default, None);
        let _ = size_submenu.append(&check_item);
        size_items.push((pct, check_item));
    }
    let _ = size_submenu.append(&PredefinedMenuItem::separator());
    let item_size_up = MenuItem::new("Increase Size (+10%) [Ctrl+Shift+=]", true, None);
    let item_size_down = MenuItem::new("Decrease Size (-10%) [Ctrl+Shift+-]", true, None);
    let _ = size_submenu.append(&item_size_up);
    let _ = size_submenu.append(&item_size_down);

    let item_quit = MenuItem::new("Quit (Ctrl+Shift+Q)", true, None);

    let _ = menu.append(&item_active);
    let _ = menu.append(&item_pres);
    let _ = menu.append(&item_mute);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&size_submenu);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item_quit);

    let mut icon_rgba = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            if dx * dx + dy * dy <= 42.0 {
                let idx = (y * 16 + x) * 4;
                icon_rgba[idx] = 255;
                icon_rgba[idx + 1] = 205;
                icon_rgba[idx + 2] = 90;
                icon_rgba[idx + 3] = 255;
            }
        }
    }
    let icon = Icon::from_rgba(icon_rgba, 16, 16).unwrap();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("TapTap")
        .with_icon(icon)
        .build()
        .unwrap();

    TrayControls {
        _tray: tray,
        item_active,
        item_pres,
        item_mute,
        size_items,
        item_size_up,
        item_size_down,
        item_quit,
    }
}

fn adjust_scale_by_step(delta: i32, controls: &TrayControls) {
    let current = USER_SCALE_PCT.load(Ordering::SeqCst) as i32;
    let next = (current + delta).clamp(50, 200) as u32;
    USER_SCALE_PCT.store(next, Ordering::SeqCst);
    controls.sync_size_checks(next);
}

fn command_from_hotkey(wparam: WPARAM) -> Option<Command> {
    match wparam.0 as i32 {
        HOTKEY_TOGGLE_ACTIVE => Some(Command::ToggleActive),
        HOTKEY_TOGGLE_PRESENTATION => Some(Command::TogglePresentation),
        HOTKEY_SIZE_UP => Some(Command::ScaleUp),
        HOTKEY_SIZE_DOWN => Some(Command::ScaleDown),
        HOTKEY_QUIT => Some(Command::Quit),
        _ => None,
    }
}

fn command_from_menu(event: &MenuEvent, controls: &TrayControls) -> Option<Command> {
    if event.id == controls.item_active.id() {
        Some(Command::ToggleActive)
    } else if event.id == controls.item_pres.id() {
        Some(Command::TogglePresentation)
    } else if event.id == controls.item_mute.id() {
        Some(Command::ToggleMute)
    } else if event.id == controls.item_size_up.id() {
        Some(Command::ScaleUp)
    } else if event.id == controls.item_size_down.id() {
        Some(Command::ScaleDown)
    } else if event.id == controls.item_quit.id() {
        Some(Command::Quit)
    } else {
        controls
            .size_items
            .iter()
            .find(|(_, item)| event.id == item.id())
            .map(|(pct, _)| Command::SetScalePreset(*pct))
    }
}

fn apply_command(command: Command, controls: &TrayControls) {
    match command {
        Command::ToggleActive => {
            let new_state = !ACTIVE.load(Ordering::SeqCst);
            ACTIVE.store(new_state, Ordering::SeqCst);
            controls.item_active.set_checked(new_state);
            if new_state {
                cursor::hide_system_cursors();
            } else {
                cursor::restore_system_cursors();
            }
        }
        Command::TogglePresentation => {
            let new_state = !PRESENTATION_MODE.load(Ordering::SeqCst);
            PRESENTATION_MODE.store(new_state, Ordering::SeqCst);
            controls.item_pres.set_checked(new_state);
        }
        Command::ToggleMute => {
            let new_state = !MUTED.load(Ordering::SeqCst);
            MUTED.store(new_state, Ordering::SeqCst);
            controls.item_mute.set_checked(new_state);
        }
        Command::ScaleUp => adjust_scale_by_step(10, controls),
        Command::ScaleDown => adjust_scale_by_step(-10, controls),
        Command::SetScalePreset(pct) => {
            USER_SCALE_PCT.store(pct, Ordering::SeqCst);
            controls.sync_size_checks(pct);
        }
        Command::Quit => {
            RUNNING.store(false, Ordering::SeqCst);
            unsafe {
                PostQuitMessage(0);
            }
        }
    }
}

fn main() {
    let _instance_mutex = unsafe {
        let mutex = CreateMutexW(None, true, w!("Local\\TapTap_SingleInstance_Mutex"))
            .expect("Failed to create single-instance mutex");

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            return;
        }
        mutex
    };

    cursor::install_failsafes();

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let mut init_pt = POINT::default();
        if GetCursorPos(&mut init_pt).is_ok() {
            MOUSE_X.store(init_pt.x, Ordering::Relaxed);
            MOUSE_Y.store(init_pt.y, Ordering::Relaxed);
        }

        let (tx, rx) = unbounded();
        let _ = EVENT_TX.set(tx);

        let instance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let class_name: PCWSTR = w!("TapTapOverlayWindowClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let ex_style = WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("TapTapOverlay"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            CANVAS_SIZE as i32,
            CANVAS_SIZE as i32,
            None,
            None,
            instance,
            None,
        )
        .expect("Failed to create overlay window");

        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0)
            .expect("Failed to install WH_MOUSE_LL hook");

        let mods = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
        let _ = RegisterHotKey(None, HOTKEY_TOGGLE_ACTIVE, mods, 0x50); // Ctrl+Shift+P
        let _ = RegisterHotKey(None, HOTKEY_TOGGLE_PRESENTATION, mods, 0x4D); // Ctrl+Shift+M
        let _ = RegisterHotKey(None, HOTKEY_QUIT, mods, 0x51); // Ctrl+Shift+Q
        let _ = RegisterHotKey(None, HOTKEY_SIZE_UP, mods, VK_OEM_PLUS.0 as u32); // Ctrl+Shift+=
        let _ = RegisterHotKey(None, HOTKEY_SIZE_DOWN, mods, VK_OEM_MINUS.0 as u32); // Ctrl+Shift+-

        let controls = create_tray_icon();
        cursor::hide_system_cursors();
        spawn_render_thread(hwnd.0 as isize, rx);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_HOTKEY {
                if let Some(command) = command_from_hotkey(msg.wParam) {
                    apply_command(command, &controls);
                }
            }

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if let Some(command) = command_from_menu(&event, &controls) {
                    apply_command(command, &controls);
                }
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        RUNNING.store(false, Ordering::SeqCst);
        let _ = UnhookWindowsHookEx(hook);
        cursor::restore_system_cursors();
    }
}