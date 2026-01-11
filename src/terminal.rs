use crate::logger;
use enigo::{Enigo, Key, Keyboard, Settings};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[cfg(not(windows))]
use sysinfo::{Pid, System};

#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, SetForegroundWindow, GetWindowTextW,
};

/// Get window title by handle (for debugging)
#[cfg(windows)]
pub fn get_window_title(hwnd: isize) -> String {
    unsafe {
        let hwnd = HWND(hwnd as *mut std::ffi::c_void);
        let mut title: [u16; 256] = [0; 256];
        let len = GetWindowTextW(hwnd, &mut title);
        String::from_utf16_lossy(&title[..len as usize])
    }
}

#[cfg(not(windows))]
pub fn get_window_title(_hwnd: isize) -> String {
    String::new()
}

/// Terminal process names to look for
const TERMINAL_PROCESS_NAMES: &[&str] = &[
    "WindowsTerminal.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "mintty.exe",
    "ConEmu64.exe",
    "ConEmu.exe",
    "alacritty.exe",
    "wezterm-gui.exe",
];

/// Find the terminal process by traversing parent processes (Windows optimized)
/// Uses Windows API directly to avoid slow full process scan
#[cfg(windows)]
pub fn find_terminal_pid() -> Option<u32> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
        PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };

    // Take a snapshot of all processes (this is fast, just creates a handle)
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()? };

    // Build a map of pid -> (parent_pid, name) by iterating once
    let mut process_map: std::collections::HashMap<u32, (u32, String)> = std::collections::HashMap::new();

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len())]
                );
                process_map.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }

    // Now traverse parent chain (fast, just map lookups)
    let mut current_pid = std::process::id();

    for _ in 0..10 {
        if let Some((parent_pid, name)) = process_map.get(&current_pid) {
            // Check if this is a terminal process
            for terminal_name in TERMINAL_PROCESS_NAMES {
                if name.eq_ignore_ascii_case(terminal_name) {
                    return Some(current_pid);
                }
            }
            current_pid = *parent_pid;
        } else {
            break;
        }
    }

    None
}

/// Find the terminal process (non-Windows fallback using sysinfo)
#[cfg(not(windows))]
pub fn find_terminal_pid() -> Option<u32> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let current_pid = Pid::from_u32(std::process::id());
    let mut current = current_pid;

    for _ in 0..10 {
        if let Some(process) = sys.process(current) {
            let name = process.name().to_string_lossy().to_string();

            for terminal_name in TERMINAL_PROCESS_NAMES {
                if name.eq_ignore_ascii_case(terminal_name) {
                    return Some(current.as_u32());
                }
            }

            if let Some(parent_pid) = process.parent() {
                current = parent_pid;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    None
}

/// Context for EnumWindows callback
#[cfg(windows)]
struct EnumWindowsContext {
    target_pid: u32,
    found_hwnd: Option<isize>,
}

/// Mutex to ensure only one get_window_by_pid call runs at a time
#[cfg(windows)]
static ENUM_WINDOWS_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
unsafe extern "system" fn enum_window_proc_by_pid(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));

    // lparam contains a pointer to EnumWindowsContext
    let context = &mut *(lparam.0 as *mut EnumWindowsContext);
    if process_id == context.target_pid {
        context.found_hwnd = Some(hwnd.0 as isize);
        return BOOL(0); // Stop enumeration
    }
    BOOL(1) // Continue enumeration
}

/// Get window handle by process ID (Windows only)
#[cfg(windows)]
pub fn get_window_by_pid(pid: u32) -> Option<isize> {
    // Lock to ensure thread safety (one call at a time)
    let _guard = ENUM_WINDOWS_LOCK.lock().unwrap();

    let mut context = EnumWindowsContext {
        target_pid: pid,
        found_hwnd: None,
    };

    // EnumWindows calls the callback synchronously, passing context via LPARAM
    unsafe {
        let _ = EnumWindows(
            Some(enum_window_proc_by_pid),
            LPARAM(&mut context as *mut EnumWindowsContext as isize),
        );
    }

    context.found_hwnd
}

#[cfg(not(windows))]
pub fn get_window_by_pid(_pid: u32) -> Option<isize> {
    None
}

/// Set the foreground window by handle
#[cfg(windows)]
pub fn set_foreground_window(hwnd: isize) -> bool {
    unsafe {
        let hwnd = HWND(hwnd as *mut std::ffi::c_void);
        SetForegroundWindow(hwnd).as_bool()
    }
}

#[cfg(not(windows))]
pub fn set_foreground_window(_hwnd: isize) -> bool {
    false
}

/// Stored terminal PID (set at startup, thread-safe)
static TERMINAL_PID: OnceLock<u32> = OnceLock::new();

/// Initialize terminal tracking at startup
/// Should be called as early as possible when the process starts
pub fn init_terminal_tracking() {
    if let Some(pid) = find_terminal_pid() {
        let _ = TERMINAL_PID.set(pid);
    }
}

/// Get the stored terminal PID
pub fn get_terminal_pid() -> Option<u32> {
    TERMINAL_PID.get().copied()
}

/// Send trigger input to the terminal
/// This function:
/// 1. Gets the terminal window handle (from override or by finding terminal process)
/// 2. Sets focus to the terminal window
/// 3. Types "//" and presses Enter
#[allow(dead_code)]
pub fn send_to_terminal(hwnd_override: Option<isize>) -> Result<(), String> {
    logger::log(&format!("[DEBUG terminal] send_to_terminal received hwnd_override: {:?}", hwnd_override));
    // Use provided hwnd if available, otherwise fall back to PID-based lookup
    let hwnd = if let Some(h) = hwnd_override {
        h
    } else {
        // Fallback: Get terminal PID and find its window
        let terminal_pid = get_terminal_pid()
            .ok_or("Terminal process not found. Was init_terminal_tracking() called?")?;
        get_window_by_pid(terminal_pid)
            .ok_or(format!("Could not find window for terminal PID {}", terminal_pid))?
    };

    logger::log(&format!("[DEBUG terminal] Using hwnd: {}", hwnd));

    // Set foreground window
    let fg_result = set_foreground_window(hwnd);
    logger::log(&format!("[DEBUG terminal] set_foreground_window result: {}", fg_result));
    if !fg_result {
        return Err("Failed to set foreground window".to_string());
    }

    // Wait for window to become active
    thread::sleep(Duration::from_millis(150));
    logger::log("[DEBUG terminal] After sleep, creating Enigo");

    // Create enigo instance for keyboard simulation
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create Enigo instance: {}", e))?;
    logger::log("[DEBUG terminal] Enigo created, typing //");

    // Type "//"
    enigo.text("//")
        .map_err(|e| format!("Failed to type text: {}", e))?;
    logger::log("[DEBUG terminal] Typed //, waiting before Enter");

    // Small delay before Enter
    thread::sleep(Duration::from_millis(50));

    // Press Enter
    logger::log("[DEBUG terminal] Pressing Enter");
    enigo.key(Key::Return, enigo::Direction::Click)
        .map_err(|e| format!("Failed to press Enter: {}", e))?;
    logger::log("[DEBUG terminal] Enter pressed, done");

    Ok(())
}

/// Send content directly to the terminal by pasting from clipboard
/// This function:
/// 1. Sets focus to the terminal window
/// 2. Simulates Ctrl+V to paste
/// 3. Presses Enter to submit
pub fn paste_to_terminal(hwnd_override: Option<isize>) -> Result<(), String> {
    logger::log(&format!("[DEBUG terminal] paste_to_terminal received hwnd_override: {:?}", hwnd_override));

    // Use provided hwnd if available, otherwise fall back to PID-based lookup
    let hwnd = if let Some(h) = hwnd_override {
        h
    } else {
        let terminal_pid = get_terminal_pid()
            .ok_or("Terminal process not found. Was init_terminal_tracking() called?")?;
        get_window_by_pid(terminal_pid)
            .ok_or(format!("Could not find window for terminal PID {}", terminal_pid))?
    };

    logger::log(&format!("[DEBUG terminal] paste_to_terminal using hwnd: {}", hwnd));

    // Set foreground window
    let fg_result = set_foreground_window(hwnd);
    logger::log(&format!("[DEBUG terminal] set_foreground_window result: {}", fg_result));
    if !fg_result {
        return Err("Failed to set foreground window".to_string());
    }

    // Wait for window to become active
    thread::sleep(Duration::from_millis(150));
    logger::log("[DEBUG terminal] After sleep, creating Enigo for paste");

    // Create enigo instance for keyboard simulation
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create Enigo instance: {}", e))?;

    // Simulate Ctrl+V to paste
    logger::log("[DEBUG terminal] Pressing Ctrl+V");
    enigo.key(Key::Control, enigo::Direction::Press)
        .map_err(|e| format!("Failed to press Ctrl: {}", e))?;
    enigo.key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| format!("Failed to press V: {}", e))?;
    enigo.key(Key::Control, enigo::Direction::Release)
        .map_err(|e| format!("Failed to release Ctrl: {}", e))?;

    logger::log("[DEBUG terminal] Ctrl+V done, waiting before Enter");

    // Small delay before Enter
    thread::sleep(Duration::from_millis(100));

    // Press Enter to submit
    logger::log("[DEBUG terminal] Pressing Enter");
    enigo.key(Key::Return, enigo::Direction::Click)
        .map_err(|e| format!("Failed to press Enter: {}", e))?;
    logger::log("[DEBUG terminal] Enter pressed, paste done");

    Ok(())
}

// Keep old function for backward compatibility
#[cfg(windows)]
pub fn get_foreground_window() -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            None
        } else {
            Some(hwnd.0 as isize)
        }
    }
}

#[cfg(not(windows))]
pub fn get_foreground_window() -> Option<isize> {
    None
}
