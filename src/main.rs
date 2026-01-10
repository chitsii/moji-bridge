mod app;
mod logger;
mod clipboard_utils;
mod hook;
mod hotkey;
mod terminal;

use clap::Parser;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Check if a MojiBridge window already exists
#[cfg(windows)]
fn check_existing_window() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe {
        let title: Vec<u16> = "MojiBridge\0".encode_utf16().collect();
        let existing = FindWindowW(None, windows::core::PCWSTR(title.as_ptr()));
        if let Ok(h) = existing {
            if !h.0.is_null() {
                return true;
            }
        }
        false
    }
}

/// Spawn the resident process detached (no console window) and exit immediately
/// CRITICAL: This function must return as fast as possible to not block Claude Code
#[cfg(windows)]
fn detach_and_spawn_resident(args: &Args) {
    use std::process::{Command, Stdio};

    // Process creation flag to hide console window
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // STEP 1: Get foreground window IMMEDIATELY (single fast API call)
    // This captures the terminal window before any delays
    let hwnd = terminal::get_foreground_window();

    // STEP 2: Check if window already exists (fast FindWindowW call)
    if check_existing_window() {
        logger::log("[DEBUG detach] MojiBridge window already exists, skipping spawn");
        return;
    }

    // STEP 3: Get exe path and build args
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };

    let mut resident_args: Vec<String> = vec!["--resident".to_string()];
    if let Some(ref label) = args.label {
        resident_args.push("--label".to_string());
        resident_args.push(label.clone());
    }
    if let Some(h) = hwnd {
        resident_args.push("--terminal-hwnd".to_string());
        resident_args.push(h.to_string());
    }

    // STEP 4: Spawn using PowerShell Start-Process for true detachment
    // Format arguments as PowerShell array: 'arg1','arg2','arg3'
    let args_str = resident_args
        .iter()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(",");
    let ps_command = format!(
        "Start-Process '{}' -ArgumentList {} -WindowStyle Hidden",
        exe_path.display(),
        args_str
    );

    logger::log(&format!("[DEBUG detach] PowerShell command: {}", ps_command));

    let mut cmd = Command::new("powershell");
    cmd.args(["-WindowStyle", "Hidden", "-Command", &ps_command]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    let _ = cmd.spawn();
    // Don't log after spawn - exit immediately
}

#[cfg(not(windows))]
fn detach_and_spawn_resident(_args: &Args) {
    // Not implemented for non-Windows
    eprintln!("Detach mode is only supported on Windows");
}

/// MojiBridge - Japanese IME Input Helper for Claude Code
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in resident mode (stay open after submit)
    #[arg(long)]
    resident: bool,

    /// Session ID from Claude Code
    #[arg(long)]
    session: Option<String>,

    /// Current working directory
    #[arg(long)]
    cwd: Option<String>,

    /// Custom label for the session
    #[arg(long)]
    label: Option<String>,

    /// Terminal window handle (passed from hook)
    #[arg(long)]
    terminal_hwnd: Option<isize>,

    /// Detach mode: spawn resident process and exit immediately
    #[arg(long)]
    detach: bool,
}

fn main() {
    // Log startup immediately
    logger::log("[DEBUG main] ===== Program started =====");

    let args = Args::parse();
    logger::log(&format!("[DEBUG main] args.resident={}, args.detach={}", args.resident, args.detach));

    // Detach mode: spawn resident process and exit immediately
    // No need for init_terminal_tracking() here - we use get_foreground_window() directly
    if args.detach {
        logger::log("[DEBUG main] Detach mode, spawning resident process");
        detach_and_spawn_resident(&args);
        logger::log("[DEBUG main] Exiting after detach");
        return;
    }

    // Initialize terminal tracking (only needed for non-detach modes)
    // This finds the terminal process by traversing parent processes
    terminal::init_terminal_tracking();

    if args.resident {
        // Resident mode: launched by SessionStart hook
        // Use hwnd from args if provided, otherwise get current foreground window
        let terminal_hwnd = args.terminal_hwnd.or_else(terminal::get_foreground_window);
        let title = terminal_hwnd.map(terminal::get_window_title).unwrap_or_default();
        logger::log(&format!("[DEBUG main] terminal_hwnd: {:?} (from args: {}), title: {}",
            terminal_hwnd, args.terminal_hwnd.is_some(), title));

        // Start global hotkey listener (Ctrl+I to focus MojiBridge when terminal is active)
        if let Some(hwnd) = terminal_hwnd {
            hotkey::set_terminal_hwnd(hwnd);
            hotkey::start_hotkey_listener();
            logger::log("[DEBUG main] Hotkey listener started");
        }

        let config = app::ResidentConfig {
            session_id: args.session.unwrap_or_default(),
            cwd: args.cwd.unwrap_or_default(),
            label: args.label,
            terminal_hwnd,
        };

        if let Err(e) = app::run_resident_gui(config) {
            eprintln!("Error running GUI: {}", e);
            std::process::exit(1);
        }
    } else {
        // Hook mode: legacy behavior for backward compatibility
        // Try to read hook input from stdin
        logger::log("[DEBUG main] Non-resident mode, reading hook input");
        match hook::read_hook_input() {
            Ok(input) => {
                logger::log(&format!("[DEBUG main] Hook input received, user_prompt: {}", input.user_prompt));
                // Check if the prompt is a trigger
                if hook::is_trigger(&input.user_prompt) {
                    logger::log("[DEBUG main] Is trigger, reading clipboard");
                    // First, try to read from clipboard (in case resident GUI sent input)
                    match clipboard_utils::read_from_clipboard() {
                        Ok(clipboard_text) => {
                            logger::log(&format!("[DEBUG main] Clipboard content: {} chars", clipboard_text.len()));
                            if !clipboard_text.trim().is_empty() {
                                // Use clipboard content as input
                                logger::log("[DEBUG main] Writing hook output with clipboard content");
                                if let Err(e) = hook::write_hook_output(&clipboard_text) {
                                    logger::log(&format!("[DEBUG main] Error writing hook output: {}", e));
                                    eprintln!("Error writing hook output: {}", e);
                                    std::process::exit(1);
                                }
                                logger::log("[DEBUG main] Hook output written successfully");
                                return;
                            }
                            logger::log("[DEBUG main] Clipboard is empty, running GUI");
                        }
                        Err(e) => {
                            logger::log(&format!("[DEBUG main] Clipboard read error: {}", e));
                        }
                    }

                    // Clipboard empty or error - run the GUI (one-shot mode)
                    if let Err(e) = app::run_gui() {
                        eprintln!("Error running GUI: {}", e);
                        std::process::exit(1);
                    }
                } else {
                    logger::log("[DEBUG main] Not a trigger, exiting silently");
                }
                // If not a trigger, exit silently (exit 0)
            }
            Err(e) => {
                logger::log(&format!("[DEBUG main] Hook input error: {}", e));
                // No input or invalid input, just run the GUI directly
                // This is useful for testing without Claude Code
                if let Err(e) = app::run_gui() {
                    eprintln!("Error running GUI: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
