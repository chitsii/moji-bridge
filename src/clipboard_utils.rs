use arboard::Clipboard;

/// Write text to the system clipboard
pub fn write_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| format!("Failed to access clipboard: {}", e))?;

    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    Ok(())
}

/// Read text from the system clipboard
pub fn read_from_clipboard() -> Result<String, String> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| format!("Failed to access clipboard: {}", e))?;

    clipboard
        .get_text()
        .map_err(|e| format!("Failed to read from clipboard: {}", e))
}
