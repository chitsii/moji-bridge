use crate::logger;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::thread;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, SetForegroundWindow, GetForegroundWindow, FindWindowW,
    MSG, WM_HOTKEY,
};

/// Virtual key code for 'I'
const VK_I: u32 = 0x49;

/// Hotkey ID
const HOTKEY_ID: i32 = 1;

/// Terminal hwnd to check (set from main thread)
static TERMINAL_HWND: AtomicIsize = AtomicIsize::new(0);

/// Set the terminal hwnd to monitor
pub fn set_terminal_hwnd(hwnd: isize) {
    TERMINAL_HWND.store(hwnd, Ordering::SeqCst);
    logger::log(&format!("[DEBUG hotkey] Terminal hwnd set to: {}", hwnd));
}

/// Start the global hotkey listener in a background thread
#[cfg(windows)]
pub fn start_hotkey_listener() {
    thread::spawn(|| {
        logger::log("[DEBUG hotkey] Starting hotkey listener thread");

        unsafe {
            // Register Ctrl+I hotkey
            let result = RegisterHotKey(
                HWND::default(), // NULL = thread-level hotkey
                HOTKEY_ID,
                MOD_CONTROL | MOD_NOREPEAT,
                VK_I,
            );

            if result.is_err() {
                logger::log("[DEBUG hotkey] Failed to register hotkey Ctrl+I");
                return;
            }

            logger::log("[DEBUG hotkey] Registered Ctrl+I hotkey successfully");

            // Message loop
            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
                if ret.0 <= 0 {
                    break;
                }

                if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == HOTKEY_ID {
                    logger::log("[DEBUG hotkey] Ctrl+I hotkey triggered");
                    handle_hotkey();
                }
            }

            // Cleanup
            let _ = UnregisterHotKey(HWND::default(), HOTKEY_ID);
            logger::log("[DEBUG hotkey] Hotkey listener thread ended");
        }
    });
}

#[cfg(not(windows))]
pub fn start_hotkey_listener() {
    // Not implemented for non-Windows
}

/// Handle the hotkey press
#[cfg(windows)]
fn handle_hotkey() {
    unsafe {
        let terminal_hwnd = TERMINAL_HWND.load(Ordering::SeqCst);
        if terminal_hwnd == 0 {
            logger::log("[DEBUG hotkey] Terminal hwnd not set");
            return;
        }

        // Get current foreground window
        let foreground = GetForegroundWindow();
        let foreground_hwnd = foreground.0 as isize;

        // Check if MojiBridge is foreground
        let claude_input_hwnd = get_claude_input_hwnd();

        logger::log(&format!(
            "[DEBUG hotkey] Foreground: {}, Terminal: {}, ClaudeInput: {:?}",
            foreground_hwnd, terminal_hwnd, claude_input_hwnd
        ));

        // Toggle between terminal and MojiBridge
        if foreground_hwnd == terminal_hwnd {
            // Terminal is foreground → focus MojiBridge
            logger::log("[DEBUG hotkey] Terminal is foreground, focusing MojiBridge");
            focus_claude_input();
        } else if Some(foreground_hwnd) == claude_input_hwnd {
            // MojiBridge is foreground → focus terminal
            logger::log("[DEBUG hotkey] MojiBridge is foreground, focusing terminal");
            focus_terminal(terminal_hwnd);
        } else {
            logger::log("[DEBUG hotkey] Other app is foreground, ignoring");
        }
    }
}

/// Get MojiBridge window handle
#[cfg(windows)]
fn get_claude_input_hwnd() -> Option<isize> {
    unsafe {
        let title: Vec<u16> = "MojiBridge\0".encode_utf16().collect();
        let hwnd = FindWindowW(None, windows::core::PCWSTR(title.as_ptr()));
        match hwnd {
            Ok(h) if !h.0.is_null() => Some(h.0 as isize),
            _ => None,
        }
    }
}

/// Focus the terminal window
#[cfg(windows)]
fn focus_terminal(hwnd: isize) {
    unsafe {
        let hwnd = HWND(hwnd as *mut std::ffi::c_void);
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Find and focus the MojiBridge window
#[cfg(windows)]
fn focus_claude_input() {
    unsafe {
        // Find window by title "MojiBridge"
        let title: Vec<u16> = "MojiBridge\0".encode_utf16().collect();
        let hwnd = FindWindowW(None, windows::core::PCWSTR(title.as_ptr()));

        match hwnd {
            Ok(h) if !h.0.is_null() => {
                logger::log(&format!("[DEBUG hotkey] Found MojiBridge window: {:?}", h.0));

                // Bring to foreground
                let result = SetForegroundWindow(h);
                logger::log(&format!("[DEBUG hotkey] SetForegroundWindow result: {}", result.as_bool()));
            }
            _ => {
                logger::log("[DEBUG hotkey] Could not find MojiBridge window");
            }
        }
    }
}
