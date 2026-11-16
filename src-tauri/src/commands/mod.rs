//! Tauri commands: the only bridge between the webview and native code.
//! Inputs are validated here; secrets never cross to the webview.

use crate::accessibility::{self, TextAccessError};
use crate::security::keychain;
use crate::shortcuts;
use crate::state::{AppState, PendingCapture};
use crate::storage::history::HistoryEntry;
use crate::storage::settings::Settings;
use crate::translation::detect::{self, Detection};
use crate::translation::engine::{self, EngineError, ErrorPayload, RetryPolicy};
use crate::translation::provider::{ModelInfo, Provider, ProviderError};
use crate::translation::registry::{self, BuildError, Preset};
use crate::translation::types::*;
use crate::translation::wordgloss;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

pub(crate) type CmdResult<T> = Result<T, ErrorPayload>;

#[cfg(feature = "coworker")]
pub mod coworker;

pub(crate) fn err(code: &str, message: impl Into<String>) -> ErrorPayload {
    ErrorPayload {
        code: code.into(),
        message: message.into(),
        retryable: false,
    }
}

fn access_err(e: TextAccessError) -> ErrorPayload {
    let code = serde_json::to_value(&e)
        .ok()
        .and_then(|v| v["code"].as_str().map(String::from))
        .unwrap_or_else(|| "access".into());
    err(&code, e.to_string())
}

/// Read a provider's API key, from memory if already read this session.
pub(crate) fn cached_key(state: &AppState, id: &str) -> Result<Option<String>, ErrorPayload> {
    if let Some(k) = state.secrets.lock().unwrap().get(id) {
        return Ok(k.clone());
    }
    let key = match keychain::get(&registry::keychain_account(id)) {
        Ok(k) => k,
        Err(_) => {
            // Denied or failed: don't ask again this session.
            state.secrets.lock().unwrap().insert(id.to_string(), None);
            return Err(err(
                "keychain",
                "macOS did not allow DigitsNepal to read your API key. Re-enter the key in Settings → AI provider (no password needed), or restart the app and choose “Always Allow”.",
            ));
        }
    };
    state.secrets.lock().unwrap().insert(id.to_string(), key.clone());
    if key.is_some() {
        remember_saved_key(state, id, true);
    }
    Ok(key)
}

fn remember_saved_key(state: &AppState, id: &str, present: bool) {
    let mut s = state.settings.lock().unwrap();
    let has = s.saved_keys.iter().any(|k| k == id);
    if present != has {
        if present {
            s.saved_keys.push(id.to_string());
        } else {
            s.saved_keys.retain(|k| k != id);
        }
        let _ = state.settings_store.save(&s);
    }
}

fn build_provider(state: &AppState, s: &Settings, id: &str) -> Result<Box<dyn Provider>, ErrorPayload> {
    let p = registry::preset(id).ok_or_else(|| err("unknown_provider", format!("Unknown provider '{id}'.")))?;
    // Local servers without a key requirement never touch the Keychain.
    let key = if p.local && !p.needs_key {
        None
    } else {
        cached_key(state, id)?
    };
    registry::build(id, &s.provider_config(id), key).map_err(|e| match e {
        BuildError::UnknownProvider(p) => err("unknown_provider", format!("Unknown provider '{p}'.")),
        BuildError::InvalidUrl(m) => err("invalid_url", m),
        BuildError::ConsentRequired(name) => err(
            "cloud_consent_required",
            format!("{name} is a cloud service: your text is sent to it for translation. Confirm this in Settings → AI provider before using it."),
        ),
        BuildError::MissingKey(name) => err("unauthorized", format!("Add your {name} API key in Settings → AI provider.")),
    })
}

// ---------- Translation ----------

#[derive(Debug, Deserialize)]
pub struct TranslateArgs {
    #[serde(flatten)]
    pub request: TranslateRequest,
    /// "No-history" for this request even if history is enabled.
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Serialize)]
pub struct TranslateResponse {
    pub result: TranslationResult,
    pub history_id: Option<String>,
}

#[tauri::command]
pub async fn translate(args: TranslateArgs, state: State<'_, AppState>) -> CmdResult<TranslateResponse> {
    let settings = state.settings.lock().unwrap().clone();
    if settings.provider_config(&settings.provider).model.trim().is_empty() {
        let name = registry::preset(&settings.provider)
            .map(|p| p.name)
            .unwrap_or("this provider");
        return Err(err(
            "no_model",
            format!("No model is selected for {name}. Choose or type one in Settings → AI provider."),
        ));
    }
    let provider = build_provider(&state, &settings, &settings.provider)?;
    let result = engine::translate(provider.as_ref(), &args.request, &RetryPolicy::default())
        .await
        .map_err(|e: EngineError| e.payload())?;
    let mut history_id = None;
    if settings.history_enabled && !args.private {
        if let Ok(e) = state.history.lock().unwrap().add(HistoryEntry::from_result(&result)) {
            history_id = Some(e.id);
        }
    }
    Ok(TranslateResponse { result, history_id })
}

#[tauri::command]
pub fn detect_language(text: String) -> Detection {
    detect::detect(&text)
}

#[tauri::command]
pub fn dictionary_gloss(text: String) -> Vec<WordMeaning> {
    wordgloss::gloss(&text)
}

#[derive(Serialize)]
pub struct ModeInfo {
    id: &'static str,
    label: &'static str,
}

#[tauri::command]
pub fn list_modes() -> Vec<ModeInfo> {
    TranslationMode::ALL
        .iter()
        .map(|m| ModeInfo {
            id: m.id(),
            label: m.label(),
        })
        .collect()
}

// ---------- Providers ----------

#[derive(Serialize)]
pub struct ProviderInfo {
    #[serde(flatten)]
    preset: &'static Preset,
    key_present: bool,
    /// Endpoint currently resolved for this provider (if valid).
    base_url: Option<String>,
    cloud: bool,
}

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    let s = state.settings.lock().unwrap().clone();
    registry::PRESETS
        .iter()
        .map(|p| {
            let cfg = s.provider_config(p.id);
            ProviderInfo {
                preset: p,
                // No Keychain read here: presence is tracked in settings.
                key_present: s.saved_keys.iter().any(|k| k == p.id),
                base_url: registry::resolve_base_url(p, &cfg).ok(),
                cloud: registry::is_cloud(p, &cfg),
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct ModelList {
    models: Vec<ModelInfo>,
    recommended: Option<String>,
}

/// Model discovery. For cloud providers this sends only the API key, but
/// it is still a request to that service, so it also requires consent.
#[tauri::command]
pub async fn list_models(provider: String, state: State<'_, AppState>) -> CmdResult<ModelList> {
    let s = state.settings.lock().unwrap().clone();
    let p = build_provider(&state, &s, &provider)?;
    let models = p
        .list_models()
        .await
        .map_err(|e: ProviderError| EngineError::Provider(e).payload())?;
    let recommended = registry::choose_default_model(&provider, &models);
    Ok(ModelList { models, recommended })
}

#[derive(Serialize)]
pub struct ProviderStatus {
    accessibility_trusted: bool,
}

#[tauri::command]
pub fn provider_status() -> ProviderStatus {
    ProviderStatus {
        accessibility_trusted: accessibility::is_trusted(false),
    }
}

#[tauri::command]
pub fn set_api_key(provider: String, key: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    registry::preset(&provider).ok_or_else(|| err("unknown_provider", "Unknown provider."))?;
    let k = keychain::validate_api_key_shape(&key).map_err(|m| err("invalid_key", m))?;
    keychain::set(&registry::keychain_account(&provider), k).map_err(|e| err("keychain", e.to_string()))?;
    state
        .secrets
        .lock()
        .unwrap()
        .insert(provider.clone(), Some(k.to_string()));
    remember_saved_key(&state, &provider, true);
    let _ = app.emit("settings:changed", state.settings.lock().unwrap().clone());
    Ok(())
}

#[tauri::command]
pub fn delete_api_key(provider: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    registry::preset(&provider).ok_or_else(|| err("unknown_provider", "Unknown provider."))?;
    keychain::delete(&registry::keychain_account(&provider)).map_err(|e| err("keychain", e.to_string()))?;
    state.secrets.lock().unwrap().insert(provider.clone(), None);
    remember_saved_key(&state, &provider, false);
    let _ = app.emit("settings:changed", state.settings.lock().unwrap().clone());
    Ok(())
}

// ---------- Settings ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn update_settings(mut next: Settings, app: AppHandle, state: State<'_, AppState>) -> CmdResult<Settings> {
    if registry::preset(&next.provider).is_none() {
        return Err(err("unknown_provider", "Unknown provider."));
    }
    for (id, cfg) in next.providers.iter_mut() {
        let p = registry::preset(id).ok_or_else(|| err("unknown_provider", format!("Unknown provider '{id}'.")))?;
        cfg.model = cfg.model.trim().to_string();
        if p.editable_url && !cfg.base_url.trim().is_empty() {
            cfg.base_url = registry::resolve_base_url(p, cfg).map_err(|m| err("invalid_url", m))?;
        } else if !p.editable_url {
            cfg.base_url.clear();
        }
    }
    next.window_opacity = next.window_opacity.clamp(30, 100);
    // Key presence is owned by the native side.
    next.saved_keys = state.settings.lock().unwrap().saved_keys.clone();
    next.shortcut = shortcuts::normalise(&next.shortcut).map_err(|m| err("invalid_shortcut", m))?;
    let prev = state.settings.lock().unwrap().clone();
    if prev.shortcut != next.shortcut || prev.shortcut_enabled != next.shortcut_enabled || prev.paused != next.paused {
        crate::register_shortcut(&app, &next).map_err(|m| err("shortcut_conflict", m))?;
    }
    if prev.history_enabled && !next.history_enabled {
        // Turning history off does not silently delete it; the UI offers "Clear all".
    }
    state.settings_store.save(&next).map_err(|e| err("io", e.to_string()))?;
    *state.settings.lock().unwrap() = next.clone();
    crate::apply_window_prefs(&app, &next);
    crate::refresh_tray(&app);
    let _ = app.emit("settings:changed", &next);
    Ok(next)
}

// ---------- History ----------

#[tauri::command]
pub fn history_search(query: String, favorites_only: bool, state: State<'_, AppState>) -> Vec<HistoryEntry> {
    state.history.lock().unwrap().search(&query, favorites_only)
}

#[tauri::command]
pub fn history_save(result: TranslationResult, state: State<'_, AppState>) -> CmdResult<HistoryEntry> {
    let mut e = HistoryEntry::from_result(&result);
    e.favorite = true;
    state
        .history
        .lock()
        .unwrap()
        .add(e)
        .map_err(|e| err("io", e.to_string()))
}

#[tauri::command]
pub fn history_favorite(id: String, favorite: bool, state: State<'_, AppState>) -> CmdResult<bool> {
    state
        .history
        .lock()
        .unwrap()
        .set_favorite(&id, favorite)
        .map_err(|e| err("io", e.to_string()))
}

#[tauri::command]
pub fn history_delete(id: String, state: State<'_, AppState>) -> CmdResult<bool> {
    state
        .history
        .lock()
        .unwrap()
        .delete(&id)
        .map_err(|e| err("io", e.to_string()))
}

#[tauri::command]
pub fn history_clear(state: State<'_, AppState>) -> CmdResult<()> {
    state
        .history
        .lock()
        .unwrap()
        .clear()
        .map_err(|e| err("io", e.to_string()))
}

/// Writes the export to ~/Downloads and returns the path.
#[tauri::command]
pub fn history_export(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    let json = state.history.lock().unwrap().export_json();
    let dir = app.path().download_dir().map_err(|e| err("io", e.to_string()))?;
    let name = format!(
        "digitsnepal-history-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = dir.join(name);
    crate::storage::write_atomic(&path, json.as_bytes()).map_err(|e| err("io", e.to_string()))?;
    Ok(path.to_string_lossy().into_owned())
}

// ---------- Clipboard (explicit user actions only) ----------

#[tauri::command]
pub fn copy_text(text: String, app: AppHandle) -> CmdResult<()> {
    crate::clipboard::write_text(&app, &text).map_err(|m| err("clipboard", m))
}

/// Called only when the user clicks a clearly labelled "Use clipboard text"
/// button or the menu-bar "Translate clipboard text" item.
#[tauri::command]
pub fn read_clipboard_text(app: AppHandle) -> CmdResult<String> {
    let t = crate::clipboard::read_text(&app).map_err(|m| err("clipboard", m))?;
    if t.trim().is_empty() {
        return Err(err("clipboard", "The clipboard is empty."));
    }
    Ok(t)
}

// ---------- Popover & selected text ----------

#[tauri::command]
pub fn get_pending_capture(state: State<'_, AppState>) -> PendingCapture {
    state.pending.lock().unwrap().clone()
}

#[derive(Serialize)]
pub struct ReplaceOutcome {
    can_undo: bool,
}

#[tauri::command]
pub async fn replace_selection(text: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<ReplaceOutcome> {
    let pid = state
        .capture
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| c.pid)
        .ok_or_else(|| access_err(TextAccessError::NoCapture))?;
    if let Some(w) = app.get_webview_window("popover") {
        let _ = w.hide();
    }
    accessibility::activate(pid);
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let outcome = {
        let guard = state.capture.lock().unwrap();
        let cap = guard.as_ref().ok_or_else(|| access_err(TextAccessError::NoCapture))?;
        accessibility::replace(cap, &text)
    };
    match outcome {
        Ok(rep) => {
            let can_undo = rep.location >= 0;
            *state.last_replacement.lock().unwrap() = Some(rep);
            // The captured selection no longer exists.
            *state.capture.lock().unwrap() = None;
            let _ = app.emit("replace:done", can_undo);
            crate::refresh_tray(&app);
            Ok(ReplaceOutcome { can_undo })
        }
        Err(e) => {
            // Bring the popover back so the user can copy instead.
            if let Some(w) = app.get_webview_window("popover") {
                let _ = w.show();
                let _ = w.set_focus();
            }
            Err(access_err(e))
        }
    }
}

#[tauri::command]
pub async fn undo_replacement(state: State<'_, AppState>) -> CmdResult<()> {
    let pid = state
        .last_replacement
        .lock()
        .unwrap()
        .as_ref()
        .map(|r| r.pid)
        .ok_or_else(|| access_err(TextAccessError::NothingToUndo))?;
    accessibility::activate(pid);
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let res = {
        let guard = state.last_replacement.lock().unwrap();
        accessibility::undo(
            guard
                .as_ref()
                .ok_or_else(|| access_err(TextAccessError::NothingToUndo))?,
        )
    };
    res.map_err(access_err)?;
    *state.last_replacement.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub fn dismiss_popover(restore_focus: bool, app: AppHandle, state: State<'_, AppState>) {
    if let Some(w) = app.get_webview_window("popover") {
        let _ = w.hide();
    }
    if restore_focus {
        if let Some(pid) = state.capture.lock().unwrap().as_ref().map(|c| c.pid) {
            accessibility::activate(pid);
        }
    }
}

#[derive(Serialize)]
pub struct AccessibilityStatus {
    trusted: bool,
}

#[tauri::command]
pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        trusted: accessibility::is_trusted(false),
    }
}

/// Shows the system prompt and opens the Accessibility settings pane.
#[tauri::command]
pub fn request_accessibility() -> AccessibilityStatus {
    let trusted = accessibility::is_trusted(true);
    if !trusted {
        accessibility::open_accessibility_settings();
    }
    AccessibilityStatus { trusted }
}

#[tauri::command]
pub fn open_main(tab: Option<String>, app: AppHandle) {
    crate::show_main(&app, tab.as_deref());
}
