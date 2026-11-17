//! App settings, stored as JSON in the app's config directory.
//! Secrets are NOT stored here (see `security::keychain`).

use crate::translation::registry::ProviderConfig;
use crate::translation::types::TranslationMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_SHORTCUT: &str = "Control+Alt+T";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Active provider preset id (see `translation::registry::PRESETS`).
    pub provider: String,
    /// Per-provider model, endpoint override and cloud consent.
    pub providers: BTreeMap<String, ProviderConfig>,
    pub default_mode: TranslationMode,
    pub shortcut: String,
    pub shortcut_enabled: bool,
    /// Opt-in local history. Off by default.
    pub history_enabled: bool,
    /// Offer "Use clipboard text" when selected text can't be read. Off by default.
    pub clipboard_fallback: bool,
    pub word_meanings: bool,
    /// Translate automatically while typing in the Translate screen.
    pub auto_translate: bool,
    /// Provider ids with an API key saved in the Keychain. Lets the UI show
    /// "key saved" without reading the Keychain (which may prompt).
    pub saved_keys: Vec<String>,
    /// Exclude DigitsNepal windows from screenshots, screen recording and
    /// screen sharing (macOS window sharing type "none").
    pub hide_from_capture: bool,
    /// Window background opacity in percent (30–100). Text stays opaque.
    pub window_opacity: u8,
    /// Pause all shortcut-driven features (menu bar toggle).
    pub paused: bool,
    /// Show the DigitsNepal button next to text fields in other apps while
    /// typing Nepali. Off by default (it checks the focused field locally).
    pub typing_button: bool,
    pub onboarding_done: bool,
}

impl Settings {
    pub fn provider_config(&self, id: &str) -> ProviderConfig {
        self.providers.get(id).cloned().unwrap_or_default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            providers: BTreeMap::new(),
            default_mode: TranslationMode::Accurate,
            shortcut: DEFAULT_SHORTCUT.into(),
            shortcut_enabled: true,
            history_enabled: false,
            clipboard_fallback: false,
            word_meanings: true,
            auto_translate: true,
            saved_keys: Vec::new(),
            hide_from_capture: false,
            window_opacity: 100,
            paused: false,
            typing_button: false,
            onboarding_done: false,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("settings.json"),
        }
    }

    /// Missing or corrupt files fall back to defaults (corrupt files are
    /// kept as `settings.json.corrupt` for inspection).
    pub fn load(&self) -> Settings {
        match std::fs::read(&self.path) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| {
                let _ = std::fs::rename(&self.path, self.path.with_extension("json.corrupt"));
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, s: &Settings) -> std::io::Result<()> {
        super::write_atomic(&self.path, &serde_json::to_vec_pretty(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_defaults() {
        let s = Settings::default();
        assert!(!s.history_enabled);
        assert!(s.providers.values().all(|c| !c.cloud_consent));
        assert!(!s.clipboard_fallback);
        assert_eq!(s.provider, "ollama", "default must be local");
        assert_eq!(s.default_mode, TranslationMode::Accurate);
    }

    #[test]
    fn roundtrip_and_partial_files() {
        let d = tempfile::tempdir().unwrap();
        let st = SettingsStore::new(d.path());
        assert_eq!(st.load(), Settings::default());
        let s = Settings {
            history_enabled: true,
            shortcut: "Control+Alt+K".into(),
            ..Default::default()
        };
        st.save(&s).unwrap();
        assert_eq!(st.load(), s);
        std::fs::write(d.path().join("settings.json"), r#"{"history_enabled": true}"#).unwrap();
        let p = st.load();
        assert!(p.history_enabled);
        assert_eq!(p.shortcut, DEFAULT_SHORTCUT);
    }

    #[test]
    fn corrupt_file_falls_back() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("settings.json"), "{{{").unwrap();
        assert_eq!(SettingsStore::new(d.path()).load(), Settings::default());
        assert!(d.path().join("settings.json.corrupt").exists());
    }
}
