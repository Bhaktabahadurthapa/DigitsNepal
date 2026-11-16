//! Tauri commands for the Sahayogi and Activity screens.
//! Folder grants are created only here, from explicit user actions.

use super::{cached_key, err, CmdResult};
use crate::coworker::ledger::{AuditEntry, Grant, PendingRow, SessionRow};
use crate::coworker::providers::{self, ProviderSpec};
use crate::coworker::service::{Coworker, UserDecision};
use crate::state::AppState;
use crate::translation::registry;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

pub struct CoworkerState(pub Arc<Coworker>);

/// `translation.translate` for agents: DigitsNepal's existing translation
/// engine with the provider and consent the user configured in Settings.
pub struct AppTranslator(pub tauri::AppHandle);

#[async_trait::async_trait]
impl crate::coworker::tools::Translator for AppTranslator {
    async fn translate(&self, text: &str, mode: &str) -> Result<serde_json::Value, String> {
        use crate::translation::engine::{self, RetryPolicy};
        use crate::translation::types::{TranslateRequest, TranslationMode};
        use tauri::Manager;
        let state = self.0.state::<AppState>();
        let settings = state.settings.lock().unwrap().clone();
        let provider = super::build_provider(&state, &settings, &settings.provider).map_err(|e| e.message)?;
        let req = TranslateRequest {
            text: text.to_string(),
            mode: TranslationMode::from_id(mode).unwrap_or_default(),
            include_word_meanings: false,
        };
        let r = engine::translate(provider.as_ref(), &req, &RetryPolicy::default())
            .await
            .map_err(|e| e.to_string())?;
        let v = serde_json::to_value(&r).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "detected_language": v["detected_language"],
            "accurate_english": v["accurate_english"],
            "styled_english": v["styled_english"],
            "warnings": v["warnings"],
        }))
    }
}

fn spec(state: &AppState) -> CmdResult<ProviderSpec> {
    let s = state.settings.lock().unwrap().clone();
    let id = s.provider.clone();
    let p = registry::preset(&id).ok_or_else(|| err("unknown_provider", "Choose an AI provider in Settings."))?;
    let key = if p.local && !p.needs_key {
        None
    } else {
        cached_key(state, &id)?
    };
    providers::spec_for(&id, &s.provider_config(&id), key).map_err(|m| err("provider_setup", m))
}

fn le(e: crate::coworker::ledger::LedgerError) -> crate::translation::engine::ErrorPayload {
    err("ledger", e.to_string())
}

#[derive(Serialize)]
pub struct CoworkerStatus {
    pub provider_ready: bool,
    pub provider_problem: Option<String>,
    pub model: Option<String>,
    pub grants: Vec<Grant>,
    pub audit_intact: bool,
}

#[tauri::command]
pub fn coworker_status(state: State<AppState>, cw: State<CoworkerState>) -> CmdResult<CoworkerStatus> {
    let (ready, problem, model) = match spec(&state) {
        Ok(s) => (true, None, Some(s.model)),
        Err(e) => (false, Some(e.message), None),
    };
    Ok(CoworkerStatus {
        provider_ready: ready,
        provider_problem: problem,
        model,
        grants: cw.0.ledger.grants().map_err(le)?,
        audit_intact: cw.0.ledger.verify_chain().map_err(le)?.is_none(),
    })
}

/// Opens the native folder picker; only a folder the user picks is granted.
/// `path` lets the user type a folder instead (still an explicit user action).
#[tauri::command]
pub async fn coworker_add_grant(
    app: tauri::AppHandle,
    path: Option<String>,
    cw: State<'_, CoworkerState>,
) -> CmdResult<Option<Grant>> {
    let chosen = match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => Some(std::path::PathBuf::from(p.trim())),
        None => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            app.dialog()
                .file()
                .set_title("Give Sahayogi access to a folder")
                .pick_folder(move |p| {
                    let _ = tx.send(p);
                });
            match rx.await.ok().flatten() {
                None => None,
                Some(fp) => Some(
                    fp.into_path()
                        .map_err(|e| err("grant", format!("Could not read the chosen folder: {e}")))?,
                ),
            }
        }
    };
    let Some(path) = chosen else { return Ok(None) };
    let g = cw.0.ledger.add_grant(&path).map_err(|e| {
        err(
            "grant",
            format!(
                "{e} ({}). If macOS asked to allow access to this folder, choose Allow and try again.",
                path.display()
            ),
        )
    })?;
    cw.0.ledger
        .record("", "user", "grant", "", &serde_json::Value::Null, "granted", &g.path)
        .map_err(le)?;
    Ok(Some(g))
}

#[tauri::command]
pub fn coworker_revoke_grant(id: i64, cw: State<CoworkerState>) -> CmdResult<()> {
    cw.0.ledger.revoke_grant(id).map_err(le)?;
    cw.0.ledger
        .record(
            "",
            "user",
            "grant",
            "",
            &serde_json::Value::Null,
            "revoked",
            &id.to_string(),
        )
        .map_err(le)?;
    Ok(())
}

#[tauri::command]
pub fn coworker_sessions(cw: State<CoworkerState>) -> CmdResult<Vec<SessionRow>> {
    cw.0.ledger.sessions().map_err(le)
}

#[derive(Serialize)]
pub struct Message {
    pub role: String,
    pub text: String,
    pub ts: i64,
}

#[tauri::command]
pub fn coworker_messages(session: String, cw: State<CoworkerState>) -> CmdResult<Vec<Message>> {
    Ok(cw
        .0
        .ledger
        .messages(&session)
        .map_err(le)?
        .into_iter()
        .map(|(role, text, ts)| Message { role, text, ts })
        .collect())
}

#[tauri::command]
pub async fn coworker_new_session(
    specialist: Option<String>,
    state: State<'_, AppState>,
    cw: State<'_, CoworkerState>,
) -> CmdResult<SessionRow> {
    let spec = spec(&state)?;
    cw.0.new_session_as(&spec, specialist.as_deref().unwrap_or("general"))
        .await
        .map_err(|e| err("coworker", e.to_string()))
}

#[tauri::command]
pub async fn coworker_send(
    session: String,
    text: String,
    state: State<'_, AppState>,
    cw: State<'_, CoworkerState>,
) -> CmdResult<()> {
    if text.chars().count() > 8000 {
        return Err(err("too_long", "Please keep a message under 8,000 characters."));
    }
    let spec = spec(&state)?;
    cw.0.send(&session, &text, &spec)
        .await
        .map_err(|e| err("coworker", e.to_string()))
}

#[tauri::command]
pub fn coworker_decide(id: String, decision: UserDecision, cw: State<CoworkerState>) -> bool {
    cw.0.decide(&id, decision)
}

#[tauri::command]
pub fn coworker_answer(id: String, text: String, cw: State<CoworkerState>) -> bool {
    cw.0.answer(&id, &text)
}

#[tauri::command]
pub async fn coworker_cancel(session: String, cw: State<'_, CoworkerState>) -> CmdResult<()> {
    cw.0.cancel(&session).await;
    Ok(())
}

#[derive(Serialize)]
pub struct Activity {
    pub entries: Vec<AuditEntry>,
    pub intact: bool,
    pub broken_at: Option<i64>,
}

#[tauri::command]
pub fn coworker_activity(limit: Option<usize>, cw: State<CoworkerState>) -> CmdResult<Activity> {
    let broken = cw.0.ledger.verify_chain().map_err(le)?;
    Ok(Activity {
        entries: cw.0.ledger.recent(limit.unwrap_or(300).min(2000)).map_err(le)?,
        intact: broken.is_none(),
        broken_at: broken,
    })
}

// ---- Connected apps (remote MCP servers) and automations -----------------

#[tauri::command]
pub fn coworker_mcp_servers(cw: State<CoworkerState>) -> Vec<crate::coworker::ledger::McpServerRow> {
    cw.0.ledger.mcp_servers()
}

#[tauri::command]
pub async fn coworker_mcp_add(
    name: String,
    url: String,
    cw: State<'_, CoworkerState>,
) -> CmdResult<crate::coworker::ledger::McpServerRow> {
    cw.0.add_mcp_server(&name, &url)
        .await
        .map_err(|e| err("mcp", e.to_string()))
}

#[tauri::command]
pub async fn coworker_mcp_enable(name: String, tools: Vec<String>, cw: State<'_, CoworkerState>) -> CmdResult<()> {
    cw.0.set_mcp_enabled(&name, tools)
        .await
        .map_err(|e| err("mcp", e.to_string()))
}

#[tauri::command]
pub async fn coworker_mcp_remove(name: String, cw: State<'_, CoworkerState>) -> CmdResult<()> {
    cw.0.remove_mcp_server(&name)
        .await
        .map_err(|e| err("mcp", e.to_string()))
}

#[tauri::command]
pub fn coworker_automations(cw: State<CoworkerState>) -> CmdResult<Vec<crate::coworker::ledger::AutomationRow>> {
    cw.0.ledger.automations().map_err(le)
}

#[tauri::command]
pub fn coworker_automation_delete(id: i64, cw: State<CoworkerState>) -> CmdResult<()> {
    cw.0.ledger.delete_automation(id).map_err(le)?;
    cw.0.ledger
        .record(
            "",
            "user",
            "automation",
            "",
            &serde_json::Value::Null,
            "deleted",
            &id.to_string(),
        )
        .map_err(le)?;
    Ok(())
}

/// The specialists a conversation can start with (for the picker).
#[tauri::command]
pub fn coworker_specialists() -> Vec<crate::coworker::specialists::Specialist> {
    crate::coworker::specialists::SPECIALISTS.to_vec()
}

#[tauri::command]
pub fn coworker_pending(cw: State<CoworkerState>) -> Vec<PendingRow> {
    cw.0.pending()
}

/// Re-show approvals left waiting by an earlier launch and continue the turn.
#[tauri::command]
pub async fn coworker_resume(
    session: String,
    state: State<'_, AppState>,
    cw: State<'_, CoworkerState>,
) -> CmdResult<bool> {
    let spec = spec(&state)?;
    cw.0.resume_pending(&session, &spec)
        .await
        .map_err(|e| err("coworker", e.to_string()))
}

#[derive(Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[tauri::command]
pub fn coworker_usage(session: Option<String>, cw: State<CoworkerState>) -> Usage {
    let (input_tokens, output_tokens) = cw.0.ledger.usage_totals(session.as_deref());
    Usage {
        input_tokens,
        output_tokens,
    }
}
