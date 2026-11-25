use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Console::{SetConsoleCtrlHandler, CTRL_CLOSE_EVENT, CTRL_C_EVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CopyIcon, CreateCursor, DestroyCursor, SetSystemCursor, SystemParametersInfoW, HCURSOR, HICON,
    OCR_APPSTARTING, OCR_CROSS, OCR_HAND, OCR_HELP, OCR_IBEAM, OCR_NO, OCR_NORMAL, OCR_SIZEALL,
    OCR_SIZENESW, OCR_SIZENS, OCR_SIZENWSE, OCR_SIZEWE, OCR_WAIT, SPIF_SENDCHANGE, SPI_SETCURSORS,
    SYSTEM_CURSOR_ID,
};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

const CURSOR_IDS: [SYSTEM_CURSOR_ID; 13] = [
    OCR_NORMAL,
    OCR_IBEAM,
    OCR_WAIT,
    OCR_CROSS,
    OCR_SIZENWSE,
    OCR_SIZENESW,
    OCR_SIZEWE,
    OCR_SIZENS,
    OCR_SIZEALL,
    OCR_NO,
    OCR_HAND,
    OCR_APPSTARTING,
    OCR_HELP,
];

pub fn install_failsafes() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_system_cursors();
        default_hook(info);
    }));

    unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> BOOL {
        if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_CLOSE_EVENT {
            restore_system_cursors();
        }
        BOOL(0)
    }

    unsafe {
        let _ = SetConsoleCtrlHandler(Some(ctrl_handler), true);
    }
}

pub fn hide_system_cursors() {
    if CURSOR_HIDDEN.swap(true, Ordering::SeqCst) {
        return;
    }

    unsafe {
        // 32x32 monochrome mask: AND mask all 1s (0xFF = transparent), XOR mask all 0s
        let and_mask = [0xFFu8; 128];
        let xor_mask = [0x00u8; 128];

        if let Ok(blank_cursor) = CreateCursor(
            None,
            0,
            0,
            32,
            32,
            and_mask.as_ptr() as *const _,
            xor_mask.as_ptr() as *const _,
        ) {
            for id in CURSOR_IDS {
                if let Ok(copied_icon) = CopyIcon(HICON(blank_cursor.0)) {
                    let _ = SetSystemCursor(HCURSOR(copied_icon.0), id);
                }
            }
            let _ = DestroyCursor(blank_cursor);
        }
    }
}

pub fn restore_system_cursors() {
    if !CURSOR_HIDDEN.swap(false, Ordering::SeqCst) {
        return;
    }
    unsafe {
        let _ = SystemParametersInfoW(SPI_SETCURSORS, 0, None, SPIF_SENDCHANGE);
    }
}
