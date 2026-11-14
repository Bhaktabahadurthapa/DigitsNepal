//! Pure, platform-independent safety rules for reading and replacing text
//! in other applications. Kept separate so they are unit-testable.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum TextAccessError {
    #[error("DigitsNepal needs Accessibility permission to read selected text.")]
    PermissionRequired,
    #[error("A password or secure field is active; DigitsNepal will not read or change it.")]
    SecureInput,
    #[error("No text is selected.")]
    NoSelection,
    #[error("This app does not expose selected text to macOS Accessibility.")]
    Unsupported,
    #[error("The selection changed since translation started, so nothing was replaced.")]
    SelectionChanged,
    #[error("The app refused the replacement. Your original text was kept.")]
    ReplaceFailed,
    #[error("Nothing to undo.")]
    NothingToUndo,
    #[error("The text was edited after replacement, so undo was skipped to avoid overwriting your changes.")]
    UndoConflict,
    #[error("There is no captured selection to replace. Copy the translation instead.")]
    NoCapture,
    #[error("Selected-text access is only available on macOS.")]
    NotMacOs,
}

const SECURE_ROLES: &[&str] = &["AXSecureTextField"];

pub fn is_secure_role(role: Option<&str>, subrole: Option<&str>) -> bool {
    role.is_some_and(|r| SECURE_ROLES.contains(&r)) || subrole.is_some_and(|r| SECURE_ROLES.contains(&r))
}

/// Selected text must exist and be non-blank to be translated.
pub fn accept_selection(text: Option<String>) -> Result<String, TextAccessError> {
    match text {
        Some(t) if !t.trim().is_empty() => Ok(t),
        Some(_) => Err(TextAccessError::NoSelection),
        None => Err(TextAccessError::Unsupported),
    }
}

/// Before replacing, the app's *current* selection must still be exactly the
/// text we captured. Otherwise the user moved on and we would overwrite the
/// wrong thing.
pub fn check_replacement(
    captured: &str,
    current_selection: Option<&str>,
    replacement: &str,
    secure: bool,
) -> Result<(), TextAccessError> {
    if secure {
        return Err(TextAccessError::SecureInput);
    }
    if replacement.trim().is_empty() {
        return Err(TextAccessError::ReplaceFailed);
    }
    match current_selection {
        Some(cur) if cur == captured => Ok(()),
        _ => Err(TextAccessError::SelectionChanged),
    }
}

/// Undo is only safe if the inserted text is still exactly where we put it.
pub fn check_undo(inserted: &str, current_text_in_range: Option<&str>) -> Result<(), TextAccessError> {
    match current_text_in_range {
        Some(cur) if cur == inserted => Ok(()),
        _ => Err(TextAccessError::UndoConflict),
    }
}

/// CFRange lengths are UTF-16 code units.
pub fn utf16_len(s: &str) -> isize {
    s.encode_utf16().count() as isize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_rules() {
        assert_eq!(accept_selection(Some("Yo ho".into())).unwrap(), "Yo ho");
        assert_eq!(accept_selection(Some("  \n".into())), Err(TextAccessError::NoSelection));
        assert_eq!(accept_selection(None), Err(TextAccessError::Unsupported));
    }

    #[test]
    fn replacement_requires_unchanged_selection() {
        assert!(check_replacement("abc", Some("abc"), "x", false).is_ok());
        assert_eq!(
            check_replacement("abc", Some("abd"), "x", false),
            Err(TextAccessError::SelectionChanged)
        );
        assert_eq!(
            check_replacement("abc", None, "x", false),
            Err(TextAccessError::SelectionChanged)
        );
        assert_eq!(
            check_replacement("abc", Some("abc"), "x", true),
            Err(TextAccessError::SecureInput)
        );
        assert_eq!(
            check_replacement("abc", Some("abc"), "  ", false),
            Err(TextAccessError::ReplaceFailed)
        );
    }

    #[test]
    fn undo_requires_untouched_text() {
        assert!(check_undo("Hello", Some("Hello")).is_ok());
        assert_eq!(check_undo("Hello", Some("Hello!")), Err(TextAccessError::UndoConflict));
        assert_eq!(check_undo("Hello", None), Err(TextAccessError::UndoConflict));
    }

    #[test]
    fn secure_roles() {
        assert!(is_secure_role(Some("AXTextField"), Some("AXSecureTextField")));
        assert!(is_secure_role(Some("AXSecureTextField"), None));
        assert!(!is_secure_role(Some("AXTextArea"), None));
    }

    #[test]
    fn utf16_lengths() {
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(utf16_len("मैले"), 4);
        assert_eq!(utf16_len("😀"), 2);
    }

    #[test]
    fn errors_serialise_with_code() {
        let v = serde_json::to_value(TextAccessError::SecureInput).unwrap();
        assert_eq!(v["code"], "secure_input");
    }
}
