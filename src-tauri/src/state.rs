use crate::accessibility::{Capture, Replacement};
use crate::storage::history::HistoryStore;
use crate::storage::settings::{Settings, SettingsStore};
use serde::Serialize;
use std::sync::Mutex;

/// What the popover should show after the shortcut fires.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PendingCapture {
    /// "ok" | "permission_required" | "secure_input" | "no_selection" | "unsupported" | "clipboard" | "paused"
    pub status: String,
    pub text: Option<String>,
    pub message: Option<String>,
    pub can_replace: bool,
    pub clipboard_fallback: bool,
    pub seq: u64,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_store: SettingsStore,
    pub history: Mutex<HistoryStore>,
    pub capture: Mutex<Option<Capture>>,
    pub last_replacement: Mutex<Option<Replacement>>,
    pub pending: Mutex<PendingCapture>,
    pub seq: Mutex<u64>,
}
