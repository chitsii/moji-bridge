use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

use crate::clipboard_utils;
use crate::logger;

/// Maximum input size to prevent DoS attacks (100KB)
const MAX_INPUT_SIZE: usize = 100 * 1024;

/// Input from Claude Code's UserPromptSubmit hook
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HookInput {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub hook_event_name: String,
    #[serde(default, rename = "prompt")]
    pub user_prompt: String,
    #[serde(default)]
    pub permission_mode: String,
}

/// Output to Claude Code's hook system (kept for future use and tests)
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "additionalContext")]
    pub additional_context: String,
}

/// Read hook input from stdin (reads single line of JSON with size limit)
pub fn read_hook_input() -> Result<HookInput, String> {
    let stdin = io::stdin();
    let mut input = String::new();

    // Read with size limit to prevent DoS attacks
    let bytes_read = stdin.lock()
        .take(MAX_INPUT_SIZE as u64)
        .read_to_string(&mut input)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;

    if bytes_read >= MAX_INPUT_SIZE {
        return Err(format!("Input too large (max {} bytes)", MAX_INPUT_SIZE));
    }

    logger::log(&format!("[DEBUG hook] Raw stdin input: {} bytes", input.len()));

    if input.trim().is_empty() {
        return Err("No input received from stdin".to_string());
    }

    serde_json::from_str(&input).map_err(|e| format!("Failed to parse JSON: {}", e))
}

/// Write hook output to stdout
/// For UserPromptSubmit, plain text stdout is added to context and shown in transcript
pub fn write_hook_output(text: &str) -> Result<(), String> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Output plain text - Claude Code will add this as context
    // The format tells Claude to treat this as the actual user request
    writeln!(handle, "[User's actual request from input helper]:\n{}", text)
        .map_err(|e| format!("Failed to write to stdout: {}", e))?;

    Ok(())
}

/// Check if the user prompt is a trigger for the input helper
pub fn is_trigger(prompt: &str) -> bool {
    prompt.trim().starts_with("//")
}

/// Write hook output with content from clipboard
/// This is used when the resident GUI has written input to clipboard
#[allow(dead_code)]
pub fn write_hook_output_from_clipboard() -> Result<(), String> {
    let clipboard_text = clipboard_utils::read_from_clipboard()?;

    if clipboard_text.trim().is_empty() {
        return Err("Clipboard is empty".to_string());
    }

    write_hook_output(&clipboard_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_trigger() {
        assert!(is_trigger("//"));
        assert!(is_trigger("// some text"));
        assert!(is_trigger("  //"));
        assert!(!is_trigger("hello"));
        assert!(!is_trigger("/hello"));
        assert!(!is_trigger(""));
    }

    #[test]
    fn test_hook_output_serialization() {
        let output = HookOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "UserPromptSubmit".to_string(),
                additional_context: "test context".to_string(),
            },
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("hookSpecificOutput"));
        assert!(json.contains("hookEventName"));
        assert!(json.contains("additionalContext"));
    }
}
