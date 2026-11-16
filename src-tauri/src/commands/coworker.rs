//! Tauri commands for the Sahayogi and Activity screens.
//! Folder grants are created only here, from explicit user actions.

use super::{cached_key, err, CmdResult};
use crate::coworker::ledger::{AuditEntry, Grant, SessionRow};
use crate::coworker::providers::{self, ProviderSpec};
use crate::coworker::service::{Coworker, UserDecision};
use crate::state::AppState;
use crate::translation::registry;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

pub struct CoworkerState(pub Arc<Coworker>);

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
pub async fn coworker_new_session(state: State<'_, AppState>, cw: State<'_, CoworkerState>) -> CmdResult<SessionRow> {
    let spec = spec(&state)?;
    cw.0.new_session(&spec)
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
