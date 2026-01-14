use crate::logger;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::thread;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, SetForegroundWindow, SetWindowsHookExW,
    ShowWindow, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, SW_RESTORE, WH_KEYBOARD_LL, WM_KEYDOWN,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
};

/// Virtual key code for 'I'
const VK_I: u32 = 0x49;

/// Terminal hwnd to monitor (set from main thread)
static TERMINAL_HWND: AtomicIsize = AtomicIsize::new(0);

/// Own MojiBridge window hwnd (set after window creation)
static OWN_MOJI_HWND: AtomicIsize = AtomicIsize::new(0);

/// Hook handle for cleanup
#[cfg(windows)]
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);

/// Set the terminal hwnd to monitor
pub fn set_terminal_hwnd(hwnd: isize) {
    TERMINAL_HWND.store(hwnd, Ordering::SeqCst);
    logger::log(&format!("[DEBUG hotkey] Terminal hwnd set to: {}", hwnd));
}

/// Set the own MojiBridge window hwnd
pub fn set_own_moji_hwnd(hwnd: isize) {
    OWN_MOJI_HWND.store(hwnd, Ordering::SeqCst);
    logger::log(&format!("[DEBUG hotkey] Own MojiBridge hwnd set to: {}", hwnd));
}

/// Get the terminal hwnd
#[allow(dead_code)]
pub fn get_terminal_hwnd() -> isize {
    TERMINAL_HWND.load(Ordering::SeqCst)
}

/// Start the keyboard hook listener in a background thread
#[cfg(windows)]
pub fn start_hotkey_listener() {
    thread::spawn(|| {
        logger::log("[DEBUG hotkey] Starting keyboard hook listener thread");

        unsafe {
            // Install low-level keyboard hook
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0);

            match hook {
                Ok(h) => {
                    HOOK_HANDLE.store(h.0 as isize, Ordering::SeqCst);
                    logger::log("[DEBUG hotkey] Keyboard hook installed successfully");

                    // Message loop to keep the hook alive
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
                        // Empty loop - just pumping messages to keep hook active
                    }

                    // Cleanup
                    let _ = UnhookWindowsHookEx(h);
                    logger::log("[DEBUG hotkey] Keyboard hook uninstalled");
                }
                Err(e) => {
                    logger::log(&format!("[DEBUG hotkey] Failed to install keyboard hook: {:?}", e));
                }
            }
        }
    });
}

#[cfg(not(windows))]
pub fn start_hotkey_listener() {
    // Not implemented for non-Windows
}

/// Low-level keyboard hook procedure
#[cfg(windows)]
unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_KEYDOWN {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);

        // Check for Ctrl+I
        if kb.vkCode == VK_I && is_ctrl_pressed() {
            let foreground = GetForegroundWindow();
            let foreground_hwnd = foreground.0 as isize;
            let terminal_hwnd = TERMINAL_HWND.load(Ordering::SeqCst);
            let own_moji_hwnd = OWN_MOJI_HWND.load(Ordering::SeqCst);

            logger::log(&format!(
                "[DEBUG hotkey] Ctrl+I detected - Foreground: {}, Terminal: {}, OwnMoji: {}",
                foreground_hwnd, terminal_hwnd, own_moji_hwnd
            ));

            // Skip if hwnd not set yet
            if terminal_hwnd == 0 {
                logger::log("[DEBUG hotkey] Terminal hwnd not set, passing through");
                return CallNextHookEx(None, code, wparam, lparam);
            }

            // Bidirectional toggle
            if foreground_hwnd == terminal_hwnd {
                // Terminal is active -> focus own MojiBridge
                logger::log("[DEBUG hotkey] Terminal is foreground, focusing MojiBridge");
                if own_moji_hwnd != 0 {
                    focus_window(own_moji_hwnd);
                    return LRESULT(1); // Consume the event
                }
            } else if own_moji_hwnd != 0 && foreground_hwnd == own_moji_hwnd {
                // Own MojiBridge is active -> focus terminal
                logger::log("[DEBUG hotkey] MojiBridge is foreground, focusing terminal");
                focus_window(terminal_hwnd);
                return LRESULT(1); // Consume the event
            }
            // Neither -> pass to next hook (other instances may handle it)
            logger::log("[DEBUG hotkey] Not our pair, passing to next hook");
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Check if Ctrl key is currently pressed
#[cfg(windows)]
fn is_ctrl_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 }
}

/// Focus a window by hwnd using Alt key simulation to bypass Windows restrictions
#[cfg(windows)]
fn focus_window(hwnd: isize) {
    unsafe {
        let target_hwnd = HWND(hwnd as *mut std::ffi::c_void);

        // Restore window if minimized
        let _ = ShowWindow(target_hwnd, SW_RESTORE);

        // Simulate Alt key press to allow SetForegroundWindow to work
        // This is a well-known workaround for Windows focus restrictions
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);

        let result = SetForegroundWindow(target_hwnd);

        // Release Alt key
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);

        logger::log(&format!(
            "[DEBUG hotkey] SetForegroundWindow result: {}",
            result.as_bool()
        ));
    }
}
