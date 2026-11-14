//! Clipboard access. Only ever triggered by an explicit user action
//! (Copy button, "Translate clipboard text", or the labelled fallback button).
//! DigitsNepal never reads the clipboard in the background and never
//! overwrites it without the user pressing Copy.

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

pub fn read_text(app: &AppHandle) -> Result<String, String> {
    app.clipboard()
        .read_text()
        .map_err(|_| "The clipboard does not contain text.".to_string())
}

pub fn write_text(app: &AppHandle, text: &str) -> Result<(), String> {
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| format!("Could not copy: {e}"))
}
