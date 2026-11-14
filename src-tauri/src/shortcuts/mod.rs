//! Global shortcut validation. Registration itself happens in `lib.rs`
//! through tauri-plugin-global-shortcut.

/// Normalise and validate a user-entered accelerator such as
/// "Control+Option+T". Returns the canonical form used by the plugin.
pub fn normalise(input: &str) -> Result<String, String> {
    let mut mods: Vec<&'static str> = Vec::new();
    let mut key: Option<String> = None;
    for raw in input.split('+').map(str::trim).filter(|s| !s.is_empty()) {
        let m = match raw.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "⌃" => Some("Control"),
            "alt" | "option" | "opt" | "⌥" => Some("Alt"),
            "shift" | "⇧" => Some("Shift"),
            "cmd" | "command" | "super" | "meta" | "⌘" => Some("Command"),
            _ => None,
        };
        match m {
            Some(m) if !mods.contains(&m) => mods.push(m),
            Some(_) => return Err(format!("'{raw}' is repeated.")),
            None => {
                if key.is_some() {
                    return Err("Use exactly one non-modifier key.".into());
                }
                let k = raw.to_ascii_uppercase();
                let valid = (k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()))
                    || (k.starts_with('F') && k[1..].parse::<u8>().is_ok_and(|n| (1..=20).contains(&n)))
                    || matches!(
                        k.as_str(),
                        "SPACE" | "ENTER" | "ESCAPE" | "TAB" | "BACKSPACE" | "SLASH" | "PERIOD" | "COMMA" | "SEMICOLON"
                    );
                if !valid {
                    return Err(format!("'{raw}' is not a supported key."));
                }
                key = Some(k);
            }
        }
    }
    let key = key.ok_or("Add a key, e.g. Control+Option+T.")?;
    let fkey = key.starts_with('F') && key.len() > 1;
    if mods.is_empty() && !fkey {
        return Err("Add at least one modifier (Control, Option, Command or Shift).".into());
    }
    if mods == ["Shift"] {
        return Err("Shift alone would capture normal typing; add Control, Option or Command.".into());
    }
    let order = ["Control", "Alt", "Shift", "Command"];
    let mut sorted: Vec<&str> = order.iter().copied().filter(|o| mods.contains(o)).collect();
    let k = key;
    sorted.push(&k);
    let canonical = sorted.join("+");
    if let Some(reason) = reserved(&canonical) {
        return Err(format!(
            "{canonical} is reserved by macOS ({reason}). Choose another shortcut."
        ));
    }
    Ok(canonical)
}

/// Well-known system or near-universal app shortcuts we refuse to take.
fn reserved(c: &str) -> Option<&'static str> {
    const R: &[(&str, &str)] = &[
        ("Command+Q", "Quit"),
        ("Command+W", "Close window"),
        ("Command+TAB", "App switcher"),
        ("Command+SPACE", "Spotlight"),
        ("Alt+Command+SPACE", "Finder search"),
        ("Control+SPACE", "Input source switching"),
        ("Control+Alt+SPACE", "Input source switching"),
        ("Command+H", "Hide app"),
        ("Alt+Command+H", "Hide others"),
        ("Command+M", "Minimise"),
        ("Command+C", "Copy"),
        ("Command+V", "Paste"),
        ("Command+X", "Cut"),
        ("Command+Z", "Undo"),
        ("Command+A", "Select all"),
        ("Command+S", "Save"),
        ("Command+T", "New tab"),
        ("Command+N", "New window"),
        ("Command+P", "Print"),
        ("Command+F", "Find"),
        ("Shift+Command+3", "Screenshot"),
        ("Shift+Command+4", "Screenshot"),
        ("Shift+Command+5", "Screenshot"),
        ("Control+Shift+Command+3", "Screenshot to clipboard"),
        ("Control+Shift+Command+4", "Screenshot to clipboard"),
        ("Control+Command+Q", "Lock screen"),
        ("Control+Command+F", "Full screen"),
        ("Control+Command+SPACE", "Emoji & Symbols"),
        ("Alt+Command+ESCAPE", "Force Quit"),
        ("Control+UP", "Mission Control"),
        ("Control+DOWN", "App windows"),
    ];
    R.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_aliases() {
        assert_eq!(normalise("Control+Option+T").unwrap(), "Control+Alt+T");
        assert_eq!(normalise("opt + ctrl + t").unwrap(), "Control+Alt+T");
        assert_eq!(normalise("cmd+shift+y").unwrap(), "Shift+Command+Y");
        assert_eq!(normalise("F6").unwrap(), "F6");
    }

    #[test]
    fn rejects_reserved_and_invalid() {
        assert!(normalise("Command+Q").unwrap_err().contains("reserved"));
        assert!(normalise("cmd+space").unwrap_err().contains("Spotlight"));
        assert!(normalise("Shift+Cmd+4").unwrap_err().contains("Screenshot"));
        assert!(normalise("T").is_err());
        assert!(normalise("Shift+T").is_err());
        assert!(normalise("Control+").is_err());
        assert!(normalise("Control+A+B").is_err());
        assert!(normalise("Control+Control+T").is_err());
        assert!(normalise("Control+Ü").is_err());
        assert!(normalise("").is_err());
    }
}
