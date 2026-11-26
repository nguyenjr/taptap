#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod cursor;
mod hand_art;
mod overlay_state;
mod physics_render;
mod platform;
mod settings;
mod spring;

use app::App;
use audio::AudioEngine;
use crossbeam_channel::{unbounded, Receiver, Sender};
use hand_art::{glove_markup, HandArtParams, HandStyle, HAND_STYLES};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use overlay_state::{OverlayState, OverlayStateSnapshot};
use physics_render::{PokeAnimator, CANVAS_SIZE, HOTSPOT_X, HOTSPOT_Y};
use platform::{Effect, Event, Hotkey, MenuAction, MouseButtonEvent, NotifyKind, Platform};
use resvg::usvg::{Options, Tree};
use settings::{Settings, SoundPreset};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Pixmap, Transform};
use tray_icon::{Icon, TrayIconBuilder};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, COLORREF, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM,
    LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GetDC,
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, ReleaseDC, SelectObject, SetBkMode,
    SetTextColor, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY, DIB_RGB_COLORS,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, HBITMAP, HDC, HFONT, HGDIOBJ,
    MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, OUT_DEFAULT_PRECIS,
    TRANSPARENT,
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

#[derive(Clone, Copy)]
enum InputEvent {
    LeftDown,
    LeftUp,
    RightEvent,
}

struct HookContext {
    state: Arc<OverlayState>,
    running: AtomicBool,
    mouse_x: AtomicI32,
    mouse_y: AtomicI32,
    input_tx: Sender<InputEvent>,
    event_tx: Sender<Event>,
    hwnd_raw: isize,
}

static HOOK_STATE: OnceLock<Arc<HookContext>> = OnceLock::new();

impl HookContext {
    fn new(
        state: Arc<OverlayState>,
        input_tx: Sender<InputEvent>,
        event_tx: Sender<Event>,
        hwnd: HWND,
        initial_cursor: POINT,
    ) -> Self {
        Self {
            state,
            running: AtomicBool::new(true),
            mouse_x: AtomicI32::new(initial_cursor.x),
            mouse_y: AtomicI32::new(initial_cursor.y),
            input_tx,
            event_tx,
            hwnd_raw: hwnd.0 as isize,
        }
    }

    fn mouse_position(&self) -> (i32, i32) {
        (
            self.mouse_x.load(Ordering::Relaxed),
            self.mouse_y.load(Ordering::Relaxed),
        )
    }
}

struct HotkeyBinding {
    hotkey: Hotkey,
    id: i32,
    vk: u32,
    label: &'static str,
    menu_label: &'static str,
}

const HOTKEY_BINDINGS: [HotkeyBinding; 5] = [
    HotkeyBinding {
        hotkey: Hotkey::ToggleActive,
        id: 1,
        vk: 0x50,
        label: "Active (Ctrl+Shift+P)",
        menu_label: "Active (Ctrl+Shift+P)",
    },
    HotkeyBinding {
        hotkey: Hotkey::TogglePresentation,
        id: 2,
        vk: 0x4D,
        label: "Presentation Mode (Ctrl+Shift+M)",
        menu_label: "Presentation Mode (Ctrl+Shift+M)",
    },
    HotkeyBinding {
        hotkey: Hotkey::Quit,
        id: 3,
        vk: 0x51,
        label: "Quit (Ctrl+Shift+Q)",
        menu_label: "Quit (Ctrl+Shift+Q)",
    },
    HotkeyBinding {
        hotkey: Hotkey::ScaleUp,
        id: 4,
        vk: VK_OEM_PLUS.0 as u32,
        label: "Increase Size (Ctrl+Shift+=)",
        menu_label: "Increase Size (+10%) [Ctrl+Shift+=]",
    },
    HotkeyBinding {
        hotkey: Hotkey::ScaleDown,
        id: 5,
        vk: VK_OEM_MINUS.0 as u32,
        label: "Decrease Size (Ctrl+Shift+-)",
        menu_label: "Decrease Size (-10%) [Ctrl+Shift+-]",
    },
];

fn notify_user(kind: NotifyKind, text: &str) {
    if cfg!(debug_assertions) && kind == NotifyKind::Failure {
        eprintln!("TapTap: {text}");
    }
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let (caption, style) = match kind {
        NotifyKind::Failure => (w!("TapTap - Error"), MB_OK | MB_ICONERROR),
        NotifyKind::Info => (w!("TapTap"), MB_OK | MB_ICONINFORMATION),
    };
    unsafe {
        let _ = MessageBoxW(None, PCWSTR(wide.as_ptr()), caption, style);
    }
}

fn duplicate_launch_message() -> &'static str {
    "TapTap is already running."
}

fn status_text() -> &'static str {
    "Presentation Mode On — clicks suppressed · Ctrl+Shift+M to exit"
}

fn hotkey_binding(hotkey: Hotkey) -> &'static HotkeyBinding {
    HOTKEY_BINDINGS
        .iter()
        .find(|binding| binding.hotkey == hotkey)
        .expect("every hotkey has a binding")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuGroup {
    Root,
    HandSize,
    HandStyle,
    ClickSound,
    Volume,
}

struct MenuBinding {
    id: &'static str,
    label: Option<&'static str>,
    hotkey: Option<Hotkey>,
    action: MenuAction,
    group: MenuGroup,
    checked: bool,
}

impl MenuBinding {
    fn label(&self) -> &str {
        if let Some(hotkey) = self.hotkey {
            return hotkey_binding(hotkey).menu_label;
        }
        match self.action {
            MenuAction::SetHand(style) => style.binding().label,
            _ => self.label.expect("menu item has a label"),
        }
    }
}

fn menu_bindings() -> Vec<MenuBinding> {
    let mut bindings = vec![
        MenuBinding {
            id: "active",
            label: None,
            hotkey: Some(Hotkey::ToggleActive),
            action: MenuAction::ToggleActive,
            group: MenuGroup::Root,
            checked: true,
        },
        MenuBinding {
            id: "presentation",
            label: None,
            hotkey: Some(Hotkey::TogglePresentation),
            action: MenuAction::TogglePresentation,
            group: MenuGroup::Root,
            checked: false,
        },
        MenuBinding {
            id: "size-up",
            label: None,
            hotkey: Some(Hotkey::ScaleUp),
            action: MenuAction::ScaleUp,
            group: MenuGroup::Root,
            checked: false,
        },
        MenuBinding {
            id: "size-down",
            label: None,
            hotkey: Some(Hotkey::ScaleDown),
            action: MenuAction::ScaleDown,
            group: MenuGroup::Root,
            checked: false,
        },
        MenuBinding {
            id: "quit",
            label: None,
            hotkey: Some(Hotkey::Quit),
            action: MenuAction::Quit,
            group: MenuGroup::Root,
            checked: false,
        },
        MenuBinding {
            id: "scale-75",
            label: Some("Small (75%)"),
            hotkey: None,
            action: MenuAction::SetScale(75),
            group: MenuGroup::HandSize,
            checked: false,
        },
        MenuBinding {
            id: "scale-100",
            label: Some("Medium (100% - Default)"),
            hotkey: None,
            action: MenuAction::SetScale(100),
            group: MenuGroup::HandSize,
            checked: true,
        },
        MenuBinding {
            id: "scale-135",
            label: Some("Large (135%)"),
            hotkey: None,
            action: MenuAction::SetScale(135),
            group: MenuGroup::HandSize,
            checked: false,
        },
        MenuBinding {
            id: "scale-175",
            label: Some("Extra Large (175%)"),
            hotkey: None,
            action: MenuAction::SetScale(175),
            group: MenuGroup::HandSize,
            checked: false,
        },
        MenuBinding {
            id: "sound-wood",
            label: Some("Wood Knock"),
            hotkey: None,
            action: MenuAction::SetSound(SoundPreset::Wood),
            group: MenuGroup::ClickSound,
            checked: true,
        },
        MenuBinding {
            id: "sound-click",
            label: Some("Click"),
            hotkey: None,
            action: MenuAction::SetSound(SoundPreset::Click),
            group: MenuGroup::ClickSound,
            checked: false,
        },
        MenuBinding {
            id: "sound-bell",
            label: Some("Bell"),
            hotkey: None,
            action: MenuAction::SetSound(SoundPreset::Bell),
            group: MenuGroup::ClickSound,
            checked: false,
        },
        MenuBinding {
            id: "sound-pop",
            label: Some("Pop"),
            hotkey: None,
            action: MenuAction::SetSound(SoundPreset::Pop),
            group: MenuGroup::ClickSound,
            checked: false,
        },
        MenuBinding {
            id: "sound-jingle",
            label: Some("Jingle Bell"),
            hotkey: None,
            action: MenuAction::SetSound(SoundPreset::JingleBell),
            group: MenuGroup::ClickSound,
            checked: false,
        },
        MenuBinding {
            id: "volume-0",
            label: Some("Off"),
            hotkey: None,
            action: MenuAction::SetVolume(0),
            group: MenuGroup::Volume,
            checked: false,
        },
        MenuBinding {
            id: "volume-25",
            label: Some("Quiet (25%)"),
            hotkey: None,
            action: MenuAction::SetVolume(25),
            group: MenuGroup::Volume,
            checked: false,
        },
        MenuBinding {
            id: "volume-50",
            label: Some("50%"),
            hotkey: None,
            action: MenuAction::SetVolume(50),
            group: MenuGroup::Volume,
            checked: false,
        },
        MenuBinding {
            id: "volume-75",
            label: Some("75%"),
            hotkey: None,
            action: MenuAction::SetVolume(75),
            group: MenuGroup::Volume,
            checked: false,
        },
        MenuBinding {
            id: "volume-100",
            label: Some("100% (Default)"),
            hotkey: None,
            action: MenuAction::SetVolume(100),
            group: MenuGroup::Volume,
            checked: true,
        },
    ];

    for (index, style) in HAND_STYLES.iter().enumerate() {
        let hand = HandStyle::from_index(index as u8);
        bindings.push(MenuBinding {
            id: style.menu_id,
            label: None,
            hotkey: None,
            action: MenuAction::SetHand(hand),
            group: MenuGroup::HandStyle,
            checked: hand == HandStyle::CLASSIC,
        });
    }
    bindings
}

fn menu_binding(action: MenuAction) -> MenuBinding {
    menu_bindings()
        .into_iter()
        .find(|binding| binding.action == action)
        .expect("every menu action has a binding")
}

fn check_item(action: MenuAction) -> CheckMenuItem {
    let binding = menu_binding(action);
    CheckMenuItem::with_id(
        MenuId::new(binding.id),
        binding.label(),
        true,
        binding.checked,
        None,
    )
}

fn plain_item(action: MenuAction) -> MenuItem {
    let binding = menu_binding(action);
    MenuItem::with_id(MenuId::new(binding.id), binding.label(), true, None)
}

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
        let Some(ctx) = HOOK_STATE.get() else {
            return CallNextHookEx(None, code, wparam, lparam);
        };
        let hook_struct = *(lparam.0 as *const MSLLHOOKSTRUCT);
        ctx.mouse_x.store(hook_struct.pt.x, Ordering::Relaxed);
        ctx.mouse_y.store(hook_struct.pt.y, Ordering::Relaxed);

        let msg = wparam.0 as u32;
        let button = if msg == WM_LBUTTONDOWN || msg == WM_LBUTTONDBLCLK {
            Some(MouseButtonEvent::LeftDown)
        } else if msg == WM_LBUTTONUP {
            Some(MouseButtonEvent::LeftUp)
        } else if msg == WM_RBUTTONDOWN || msg == WM_RBUTTONUP || msg == WM_RBUTTONDBLCLK {
            Some(MouseButtonEvent::Right)
        } else {
            None
        };

        if let Some(button) = button {
            if ctx.state.snapshot().active {
                let animation = match button {
                    MouseButtonEvent::LeftDown => InputEvent::LeftDown,
                    MouseButtonEvent::LeftUp => InputEvent::LeftUp,
                    MouseButtonEvent::Right => InputEvent::RightEvent,
                };
                let _ = ctx.input_tx.try_send(animation);
            }

            let _ = ctx.event_tx.try_send(Event::MouseButton(button));
            unsafe {
                let _ = PostMessageW(HWND(ctx.hwnd_raw as *mut _), WM_APP, WPARAM(0), LPARAM(0));
            }
            if ctx.state.suppresses_clicks() {
                return LRESULT(1);
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

struct RenderResources {
    screen_dc: HDC,
    mem_dc: HDC,
    hbitmap: HBITMAP,
    old_obj: HGDIOBJ,
}

impl Drop for RenderResources {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.mem_dc, self.old_obj);
            let _ = DeleteObject(self.hbitmap);
            let _ = DeleteDC(self.mem_dc);
            let _ = ReleaseDC(None, self.screen_dc);
        }
    }
}

fn rounded_rect_path(width: f32, height: f32, radius: f32) -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(radius, 0.0);
    builder.line_to(width - radius, 0.0);
    builder.quad_to(width, 0.0, width, radius);
    builder.line_to(width, height - radius);
    builder.quad_to(width, height, width - radius, height);
    builder.line_to(radius, height);
    builder.quad_to(0.0, height, 0.0, height - radius);
    builder.line_to(0.0, radius);
    builder.quad_to(0.0, 0.0, radius, 0.0);
    builder.close();
    builder.finish()
}

struct StatusWindow {
    hwnd: HWND,
    screen_dc: HDC,
    mem_dc: HDC,
    hbitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    font: HFONT,
    old_font: HGDIOBJ,
    bits_ptr: *mut u8,
    width: usize,
    height: usize,
    dst_pt: POINT,
    size: SIZE,
    blend: BLENDFUNCTION,
}

impl StatusWindow {
    fn new(class_name: PCWSTR, instance: HINSTANCE) -> Result<Self, String> {
        unsafe {
            let monitor = MonitorFromWindow(None, MONITOR_DEFAULTTOPRIMARY);
            let mut monitor_info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
                return Err("Could not locate the primary display.".to_string());
            }

            let work = monitor_info.rcWork;
            let dpi =
                query_monitor_dpi_scale((work.left + work.right) / 2, (work.top + work.bottom) / 2);
            let width = (640.0 * dpi) as i32;
            let height = (46.0 * dpi) as i32;
            let margin = (18.0 * dpi) as i32;
            let dst_pt = POINT {
                x: work.right - width - margin,
                y: work.bottom - height - margin,
            };
            let size = SIZE {
                cx: width,
                cy: height,
            };

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                class_name,
                w!("TapTapStatus"),
                WS_POPUP,
                dst_pt.x,
                dst_pt.y,
                width,
                height,
                None,
                None,
                instance,
                None,
            )
            .map_err(|error| format!("Could not create the status window: {error:?}"))?;

            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbitmap = match CreateDIBSection(
                mem_dc,
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut bits,
                HANDLE::default(),
                0,
            ) {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    let _ = DestroyWindow(hwnd);
                    return Err(format!("Could not allocate the status surface: {error:?}"));
                }
            };
            let old_bitmap = SelectObject(mem_dc, hbitmap);

            let font = CreateFontW(
                (16.0 * dpi) as i32,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                DEFAULT_QUALITY.0 as u32,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                w!("Segoe UI"),
            );
            if font.0.is_null() {
                let _ = SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(hbitmap);
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(None, screen_dc);
                let _ = DestroyWindow(hwnd);
                return Err("Could not create the status font.".to_string());
            }
            let old_font = SelectObject(mem_dc, font);

            Ok(Self {
                hwnd,
                screen_dc,
                mem_dc,
                hbitmap,
                old_bitmap,
                font,
                old_font,
                bits_ptr: bits as *mut u8,
                width: width as usize,
                height: height as usize,
                dst_pt,
                size,
                blend: BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                },
            })
        }
    }

    fn render(&mut self) {
        let pixel_count = self.width * self.height;
        let byte_count = pixel_count * 4;
        unsafe {
            std::ptr::write_bytes(self.bits_ptr, 0, byte_count);
            let mut text: Vec<u16> = status_text().encode_utf16().collect();
            let padding = (self.height / 5) as i32;
            let mut text_rect = RECT {
                left: padding,
                top: 0,
                right: self.width as i32 - padding,
                bottom: self.height as i32,
            };
            SetTextColor(self.mem_dc, COLORREF(0x00FFFFFF));
            SetBkMode(self.mem_dc, TRANSPARENT);
            DrawTextW(
                self.mem_dc,
                &mut text,
                &mut text_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            let mask = std::slice::from_raw_parts(self.bits_ptr, byte_count).to_vec();

            let mut pixmap = Pixmap::new(self.width as u32, self.height as u32)
                .expect("Failed to allocate status pixmap");
            pixmap.fill(Color::TRANSPARENT);
            if let Some(path) = rounded_rect_path(
                self.width as f32,
                self.height as f32,
                (self.height as f32) * 0.28,
            ) {
                let mut paint = Paint::default();
                paint.set_color_rgba8(18, 18, 26, 215);
                paint.anti_alias = true;
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::default(), None);
            }

            let pixels = pixmap.data_mut();
            for index in 0..pixel_count {
                let offset = index * 4;
                let coverage = mask[offset + 2] as f32 / 255.0;
                if coverage == 0.0 {
                    continue;
                }
                let panel_alpha = pixels[offset + 3] as f32 / 255.0;
                let out_alpha = coverage + panel_alpha * (1.0 - coverage);
                for channel in 0..3 {
                    let panel = pixels[offset + channel] as f32 / 255.0;
                    let blended = (coverage + panel * panel_alpha * (1.0 - coverage)) / out_alpha;
                    pixels[offset + channel] = (blended * 255.0) as u8;
                }
                pixels[offset + 3] = (out_alpha * 255.0) as u8;
            }

            std::ptr::copy_nonoverlapping(pixmap.data().as_ptr(), self.bits_ptr, byte_count);
            for pixel in std::slice::from_raw_parts_mut(self.bits_ptr, byte_count)
                .as_chunks_mut::<4>()
                .0
            {
                pixel.swap(0, 2);
            }

            let source_point = POINT { x: 0, y: 0 };
            let _ = UpdateLayeredWindow(
                self.hwnd,
                self.screen_dc,
                Some(&self.dst_pt),
                Some(&self.size),
                self.mem_dc,
                Some(&source_point),
                COLORREF(0),
                Some(&self.blend),
                ULW_ALPHA,
            );
        }
    }

    fn show(&mut self) {
        self.render();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    fn hide(&mut self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }
}

impl Drop for StatusWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.mem_dc, self.old_font);
            let _ = SelectObject(self.mem_dc, self.old_bitmap);
            let _ = DeleteObject(self.font);
            let _ = DeleteObject(self.hbitmap);
            let _ = DeleteDC(self.mem_dc);
            let _ = ReleaseDC(None, self.screen_dc);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn spawn_render_thread(
    hwnd_raw: isize,
    rx: Receiver<InputEvent>,
    error_tx: Sender<String>,
    ctx: Arc<HookContext>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut _);
        let mut animator = PokeAnimator::new();
        let audio = AudioEngine::new();

        let (resources, bits_ptr) = unsafe {
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
            match CreateDIBSection(
                mem_dc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                HANDLE::default(),
                0,
            ) {
                Ok(hbitmap) => {
                    let old_obj = SelectObject(mem_dc, hbitmap);
                    (
                        Ok(RenderResources {
                            screen_dc,
                            mem_dc,
                            hbitmap,
                            old_obj,
                        }),
                        bits_ptr,
                    )
                }
                Err(error) => {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    (Err(error), bits_ptr)
                }
            }
        };

        let (resources, bits_ptr) = match resources {
            Ok(resources) => (resources, bits_ptr),
            Err(error) => {
                if cfg!(debug_assertions) {
                    eprintln!("TapTap: render surface error: {error:?}");
                }
                let _ = error_tx.send("Could not create the render surface.".to_string());
                unsafe {
                    let _ = PostMessageW(hwnd, WM_APP, WPARAM(0), LPARAM(0));
                }
                return;
            }
        };

        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let mut last_tick = Instant::now();
            let mut frame_count: u32 = 0;

            while ctx.running.load(Ordering::Relaxed) {
                let now = Instant::now();
                let dt = (now - last_tick).as_secs_f32().min(0.05);
                last_tick = now;

                let mut force_topmost = false;

                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        InputEvent::LeftDown => {
                            animator.trigger_click(true);
                            let snapshot = ctx.state.snapshot();
                            audio.play_poke(animator.combo_heat, snapshot.sound, snapshot.volume);
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

                let snapshot = ctx.state.snapshot();
                if !snapshot.active {
                    std::thread::sleep(Duration::from_millis(16));
                    continue;
                }

                frame_count = frame_count.wrapping_add(1);
                if force_topmost || frame_count.is_multiple_of(10) {
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

                let (mx, my) = ctx.mouse_position();
                let dpi_scale = query_monitor_dpi_scale(mx, my);
                let user_scale = snapshot.scale_pct as f32 / 100.0;

                animator.step(dt, user_scale);
                let pixels = animator.render_bgra(dpi_scale, user_scale, snapshot.hand_style);

                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits_ptr as *mut u8, pixels.len());

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
                    resources.screen_dc,
                    Some(&dst_pt),
                    Some(&wnd_size),
                    resources.mem_dc,
                    Some(&src_pt),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );

                std::thread::sleep(Duration::from_millis(8));
            }
        }
        drop(resources);
    })
}

struct TrayControls {
    _tray: tray_icon::TrayIcon,
    item_active: CheckMenuItem,
    item_pres: CheckMenuItem,
    size_items: Vec<(u32, CheckMenuItem)>,
    hand_items: Vec<(HandStyle, CheckMenuItem)>,
    sound_items: Vec<(SoundPreset, CheckMenuItem)>,
    volume_items: Vec<(u8, CheckMenuItem)>,
}

impl TrayControls {
    fn sync_all(&self, snapshot: OverlayStateSnapshot) {
        self.item_active.set_checked(snapshot.active);
        self.item_pres.set_checked(snapshot.presentation);
        for (pct, item) in &self.size_items {
            item.set_checked(*pct == snapshot.scale_pct);
        }
        for (style, item) in &self.hand_items {
            item.set_checked(*style == snapshot.hand_style);
        }
        for (preset, item) in &self.sound_items {
            item.set_checked(*preset == snapshot.sound);
        }
        for (level, item) in &self.volume_items {
            item.set_checked(*level == snapshot.volume);
        }
    }
}

fn create_tray_icon() -> TrayControls {
    let menu = Menu::new();
    let bindings = menu_bindings();
    let item_active = check_item(MenuAction::ToggleActive);
    let item_pres = check_item(MenuAction::TogglePresentation);
    let _ = menu.append(&item_active);
    let _ = menu.append(&item_pres);
    let _ = menu.append(&PredefinedMenuItem::separator());

    let size_submenu = Submenu::new("Hand Size", true);
    let size_items: Vec<(u32, CheckMenuItem)> = bindings
        .iter()
        .filter(|binding| binding.group == MenuGroup::HandSize)
        .filter_map(|binding| match binding.action {
            MenuAction::SetScale(pct) => Some((pct, check_item(binding.action))),
            _ => None,
        })
        .collect();
    for (_, item) in &size_items {
        let _ = size_submenu.append(item);
    }
    let _ = size_submenu.append(&PredefinedMenuItem::separator());
    let item_size_up = plain_item(MenuAction::ScaleUp);
    let item_size_down = plain_item(MenuAction::ScaleDown);
    let _ = size_submenu.append(&item_size_up);
    let _ = size_submenu.append(&item_size_down);

    let hand_submenu = Submenu::new("Hand Style", true);
    let hand_items: Vec<(HandStyle, CheckMenuItem)> = bindings
        .iter()
        .filter(|binding| binding.group == MenuGroup::HandStyle)
        .filter_map(|binding| match binding.action {
            MenuAction::SetHand(style) => Some((style, check_item(binding.action))),
            _ => None,
        })
        .collect();
    for (_, item) in &hand_items {
        let _ = hand_submenu.append(item);
    }

    let sound_submenu = Submenu::new("Click Sound", true);
    let sound_items: Vec<(SoundPreset, CheckMenuItem)> = bindings
        .iter()
        .filter(|binding| binding.group == MenuGroup::ClickSound)
        .filter_map(|binding| match binding.action {
            MenuAction::SetSound(preset) => Some((preset, check_item(binding.action))),
            _ => None,
        })
        .collect();
    for (_, item) in &sound_items {
        let _ = sound_submenu.append(item);
    }

    let volume_submenu = Submenu::new("Volume", true);
    let volume_items: Vec<(u8, CheckMenuItem)> = bindings
        .iter()
        .filter(|binding| binding.group == MenuGroup::Volume)
        .filter_map(|binding| match binding.action {
            MenuAction::SetVolume(level) => Some((level, check_item(binding.action))),
            _ => None,
        })
        .collect();
    for (_, item) in &volume_items {
        let _ = volume_submenu.append(item);
    }

    let item_quit = plain_item(MenuAction::Quit);

    let _ = menu.append(&size_submenu);
    let _ = menu.append(&hand_submenu);
    let _ = menu.append(&sound_submenu);
    let _ = menu.append(&volume_submenu);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item_quit);

    let icon_svg = format!(
        r##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="20 6 182 182" width="32" height="32">
  {}
</svg>
"##,
        glove_markup(HandArtParams::new(10.0, 7.0, 7.0))
    );
    let tree = Tree::from_str(&icon_svg, &Options::default()).expect("Valid tray icon SVG");
    let mut pixmap = Pixmap::new(32, 32).expect("Failed to allocate tray icon pixmap");
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
    let icon = Icon::from_rgba(pixmap.data().to_vec(), 32, 32).expect("Failed to build tray icon");

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
        size_items,
        hand_items,
        sound_items,
        volume_items,
    }
}

fn hotkey_from_wparam(wparam: WPARAM) -> Option<Hotkey> {
    HOTKEY_BINDINGS
        .iter()
        .find(|binding| binding.id == wparam.0 as i32)
        .map(|binding| binding.hotkey)
}

fn menu_action(event: &MenuEvent) -> Option<MenuAction> {
    menu_bindings()
        .iter()
        .find(|binding| event.id == binding.id)
        .map(|binding| binding.action)
}

fn register_hotkeys() -> Vec<String> {
    let mods = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
    let mut failures = Vec::new();
    unsafe {
        for binding in HOTKEY_BINDINGS {
            if RegisterHotKey(None, binding.id, mods, binding.vk).is_err() {
                failures.push(format!(
                    "Could not register the shortcut: {}.",
                    binding.label
                ));
            }
        }
    }
    failures
}

struct HookGuard(HHOOK);

impl Drop for HookGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}

struct Win32Platform {
    hwnd: HWND,
    controls: TrayControls,
    msg: MSG,
    pending: VecDeque<Event>,
    state: Arc<OverlayState>,
    hook_ctx: Arc<HookContext>,
    input_rx: Receiver<InputEvent>,
    event_rx: Receiver<Event>,
    error_rx: Receiver<String>,
    error_tx: Sender<String>,
    startup_failures: Vec<String>,
    status: Option<StatusWindow>,
    hook: Option<HookGuard>,
    render_thread: Option<JoinHandle<()>>,
}

impl Win32Platform {
    fn new(
        hwnd: HWND,
        controls: TrayControls,
        initial_cursor: POINT,
        state: Arc<OverlayState>,
        status: Option<StatusWindow>,
        initial_failures: Vec<String>,
    ) -> Self {
        let (input_tx, input_rx) = unbounded();
        let (event_tx, event_rx) = unbounded::<Event>();
        let (error_tx, error_rx) = unbounded::<String>();
        let hook_ctx = Arc::new(HookContext::new(
            state.clone(),
            input_tx,
            event_tx,
            hwnd,
            initial_cursor,
        ));
        let _ = HOOK_STATE.set(hook_ctx.clone());

        let pending = VecDeque::new();

        Self {
            hwnd,
            controls,
            msg: MSG::default(),
            pending,
            state,
            hook_ctx,
            input_rx,
            event_rx,
            error_rx,
            error_tx,
            startup_failures: initial_failures,
            status,
            hook: None,
            render_thread: None,
        }
    }

    fn collect_pending(&mut self) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(action) = menu_action(&event) {
                self.pending.push_back(Event::Menu(action));
            }
        }
        while let Ok(event) = self.event_rx.try_recv() {
            self.pending.push_back(event);
        }
        while let Ok(error) = self.error_rx.try_recv() {
            self.pending.push_back(Event::RenderFailed(error));
        }
    }
}

impl Platform for Win32Platform {
    fn next_event(&mut self) -> Option<Event> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }

        loop {
            let status = unsafe { GetMessageW(&mut self.msg, None, 0, 0) };
            if status.0 == -1 {
                return Some(Event::MessageLoopFailed);
            }
            if status.0 == 0 {
                return None;
            }

            if self.msg.message == WM_HOTKEY {
                if let Some(hotkey) = hotkey_from_wparam(self.msg.wParam) {
                    self.collect_pending();
                    return Some(Event::Hotkey(hotkey));
                }
            }

            unsafe {
                let _ = TranslateMessage(&self.msg);
                DispatchMessageW(&self.msg);
            }

            self.collect_pending();
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
        }
    }

    fn emit(&mut self, effect: Effect) {
        match effect {
            Effect::RegisterHotkeys => {
                let failures = register_hotkeys();
                if cfg!(debug_assertions) {
                    for failure in &failures {
                        eprintln!("TapTap: {failure}");
                    }
                }
                self.startup_failures.extend(failures);
            }
            Effect::InstallMouseHook => unsafe {
                let instance = GetModuleHandleW(None).expect("Failed to get module handle");
                match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0) {
                    Ok(hook) => self.hook = Some(HookGuard(hook)),
                    Err(error) => {
                        if cfg!(debug_assertions) {
                            eprintln!("TapTap: mouse hook error: {error:?}");
                        }
                        self.startup_failures
                            .push("Could not install the mouse hook.".to_string());
                    }
                }
            },
            Effect::RemoveMouseHook => self.hook = None,
            Effect::StartRenderThread => {
                let handle = spawn_render_thread(
                    self.hwnd.0 as isize,
                    self.input_rx.clone(),
                    self.error_tx.clone(),
                    self.hook_ctx.clone(),
                );
                self.render_thread = Some(handle);
                if !self.startup_failures.is_empty() {
                    let messages = std::mem::take(&mut self.startup_failures);
                    self.pending.push_back(Event::StartupFailed(messages));
                }
            }
            Effect::StopRenderThread => {
                self.hook_ctx.running.store(false, Ordering::SeqCst);
                if let Some(handle) = self.render_thread.take() {
                    let _ = handle.join();
                }
            }
            Effect::HideSystemCursors => cursor::hide_system_cursors(),
            Effect::RestoreSystemCursors => cursor::restore_system_cursors(),
            Effect::StateChanged => {
                let snapshot = self.state.snapshot();
                self.controls.sync_all(snapshot);
                unsafe {
                    if snapshot.active {
                        cursor::hide_system_cursors();
                        let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                    } else {
                        cursor::restore_system_cursors();
                        let _ = ShowWindow(self.hwnd, SW_HIDE);
                    }
                }
                if let Some(status) = &mut self.status {
                    if snapshot.active && snapshot.presentation {
                        status.show();
                    } else {
                        status.hide();
                    }
                }
            }
            Effect::PostQuit => unsafe {
                PostQuitMessage(0);
            },
            Effect::Notify { kind, text } => notify_user(kind, &text),
            Effect::LoadSettings => match Settings::load() {
                Ok(settings) => self.pending.push_back(Event::SettingsLoaded(settings)),
                Err(error) => {
                    if cfg!(debug_assertions) {
                        eprintln!("TapTap: settings load error: {error}");
                    }
                    self.pending.push_back(Event::SettingsLoadFailed(
                        "Could not read the settings file.".to_string(),
                    ));
                }
            },
            Effect::SaveSettings(settings) => {
                if let Err(error) = settings.save() {
                    if cfg!(debug_assertions) {
                        eprintln!("TapTap: settings save error: {error}");
                    }
                    self.pending.push_back(Event::SettingsSaveFailed(
                        "Could not save the settings file.".to_string(),
                    ));
                }
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
            notify_user(NotifyKind::Info, duplicate_launch_message());
            return;
        }
        mutex
    };

    cursor::install_failsafes();

    let mut init_pt = POINT { x: 500, y: 500 };
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_ok() {
            init_pt = cursor;
        }
    }

    unsafe {
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

        let ex_style =
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;

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

        let controls = create_tray_icon();
        let mut initial_failures = Vec::new();
        let status = match StatusWindow::new(class_name, instance) {
            Ok(status) => Some(status),
            Err(error) => {
                if cfg!(debug_assertions) {
                    eprintln!("TapTap: {error}");
                }
                initial_failures
                    .push("Could not create the Presentation status window.".to_string());
                None
            }
        };
        let state = Arc::new(OverlayState::new());
        let mut platform = Win32Platform::new(
            hwnd,
            controls,
            init_pt,
            state.clone(),
            status,
            initial_failures,
        );
        let mut app = App::new(state);

        app.handle(Event::Start, &mut platform);
        app.run(&mut platform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

    #[test]
    fn duplicate_launch_message_names_the_running_instance() {
        assert!(duplicate_launch_message().contains("already running"));
    }

    #[test]
    fn status_text_names_the_state_and_exit_hotkey() {
        let text = status_text();
        assert!(text.contains("Presentation Mode On"));
        assert!(text.contains("clicks suppressed"));
        assert!(text.contains("Ctrl+Shift+M"));
    }

    fn test_window() -> HWND {
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
            let class_name = w!("TapTapMessageLoopTestClass");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance,
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassExW(&wc);
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name,
                w!("test"),
                WS_OVERLAPPED,
                0,
                0,
                10,
                10,
                None,
                None,
                instance,
                None,
            )
            .unwrap()
        }
    }

    fn tray_window() -> HWND {
        unsafe { FindWindowW(w!("tray_icon_app"), None).expect("tray window") }
    }

    fn drain_messages() {
        loop {
            let mut msg = MSG::default();
            unsafe {
                if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).0 == 0 {
                    return;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    #[test]
    fn every_hand_style_has_a_menu_binding() {
        let bindings = menu_bindings();
        for (index, style) in HAND_STYLES.iter().enumerate() {
            let action = MenuAction::SetHand(HandStyle::from_index(index as u8));
            assert!(bindings.iter().any(|binding| {
                binding.group == MenuGroup::HandStyle
                    && binding.action == action
                    && binding.id == style.menu_id
                    && binding.label() == style.label
            }));
        }
    }

    #[test]
    fn menu_bindings_map_every_item_to_an_action() {
        for binding in menu_bindings() {
            let event = MenuEvent {
                id: MenuId::new(binding.id),
            };
            assert_eq!(menu_action(&event), Some(binding.action));
        }
    }

    fn quit_command_id(hwnd: HWND) -> u32 {
        let quit_id = menu_binding(MenuAction::Quit).id;
        for id in 1000..1200u32 {
            unsafe {
                let _ = PostMessageW(hwnd, WM_COMMAND, WPARAM(id as usize), LPARAM(0));
            }
            drain_messages();
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == quit_id {
                    return id;
                }
            }
        }
        panic!("quit menu command id not found");
    }

    #[test]
    fn menu_quit_event_is_delivered_on_the_first_message() {
        let hwnd = test_window();
        let controls = create_tray_icon();
        let tray_hwnd = tray_window();
        let quit_id = quit_command_id(tray_hwnd);
        let mut platform = Win32Platform::new(
            hwnd,
            controls,
            POINT { x: 500, y: 500 },
            Arc::new(OverlayState::new()),
            None,
            Vec::new(),
        );

        let thread_id = unsafe { GetCurrentThreadId() };
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        });

        unsafe {
            let _ = PostMessageW(tray_hwnd, WM_COMMAND, WPARAM(quit_id as usize), LPARAM(0));
        }

        let event = platform.next_event();
        assert!(
            matches!(event, Some(Event::Menu(MenuAction::Quit))),
            "got {event:?}"
        );
    }
}
