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

#[derive(Serialize)]
pub struct PendingItem {
    #[serde(flatten)]
    pub row: PendingRow,
    /// Answerable now; otherwise the chat must be reopened first.
    pub live: bool,
}

#[tauri::command]
pub fn coworker_pending(cw: State<CoworkerState>) -> Vec<PendingItem> {
    cw.0.pending()
        .into_iter()
        .map(|row| PendingItem {
            live: cw.0.is_live(&row.id),
            row,
        })
        .collect()
}

// ---- Tasks ----------------------------------------------------------------------

#[derive(Serialize)]
pub struct TaskPreview {
    pub hints: crate::coworker::intent::IntentHints,
    pub workflow: Option<String>,
    pub stop_at: Option<String>,
}

/// Offline, instant understanding of a request while the user types.
#[tauri::command]
pub fn coworker_task_preview(text: String) -> TaskPreview {
    let hints = crate::coworker::intent::analyze(&text);
    let pb = crate::coworker::tasks::playbook(&hints.category);
    TaskPreview {
        workflow: pb.map(|p| p.title.to_string()),
        stop_at: pb.map(|p| p.stop_at.to_string()),
        hints,
    }
}

#[tauri::command]
pub async fn coworker_task_create(
    text: String,
    specialist: Option<String>,
    auto_web: Option<bool>,
    state: State<'_, AppState>,
    cw: State<'_, CoworkerState>,
) -> CmdResult<crate::coworker::ledger::TaskRow> {
    if text.chars().count() > 4000 {
        return Err(err("too_long", "Please keep a task under 4,000 characters."));
    }
    let spec = spec(&state)?;
    cw.0.create_task(&spec, &text, specialist.as_deref(), auto_web.unwrap_or(false))
        .await
        .map_err(|e| err("task", e.to_string()))
}

#[tauri::command]
pub fn coworker_tasks(cw: State<CoworkerState>) -> Vec<crate::coworker::ledger::TaskRow> {
    cw.0.recover_tasks();
    cw.0.ledger.tasks()
}

#[derive(Serialize)]
pub struct TaskDetail {
    pub task: crate::coworker::ledger::TaskRow,
    pub events: Vec<serde_json::Value>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub interventions: i64,
}

#[tauri::command]
pub fn coworker_task(id: String, cw: State<CoworkerState>) -> CmdResult<TaskDetail> {
    let task = cw.0.ledger.task(&id).ok_or_else(|| err("task", "Unknown task."))?;
    let (input_tokens, output_tokens) = cw.0.ledger.usage_totals(Some(&task.session));
    Ok(TaskDetail {
        events: cw.0.ledger.task_events(&id),
        interventions: cw.0.ledger.session_interventions(&task.session),
        input_tokens,
        output_tokens,
        task,
    })
}

#[tauri::command]
pub async fn coworker_task_control(
    id: String,
    action: String,
    state: State<'_, AppState>,
    cw: State<'_, CoworkerState>,
) -> CmdResult<()> {
    let r = match action.as_str() {
        "pause" => cw.0.pause_task(&id).await,
        "cancel" => cw.0.cancel_task(&id).await,
        "resume" => cw.0.resume_task(&id, &spec(&state)?).await,
        "retry" => cw.0.retry_task(&id, &spec(&state)?).await,
        _ => return Err(err("task", "Unknown action.")),
    };
    r.map_err(|e| err("task", e.to_string()))
}

#[derive(Serialize)]
pub struct TaskMetrics {
    pub total: usize,
    pub completed: usize,
    pub verified: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub active: usize,
    pub completion_rate: Option<f64>,
    pub verified_rate: Option<f64>,
    pub interventions_per_task: Option<f64>,
    pub tokens_per_task: Option<f64>,
    pub median_minutes: Option<f64>,
}

/// Honest numbers from the local records only.
#[tauri::command]
pub fn coworker_task_metrics(cw: State<CoworkerState>) -> TaskMetrics {
    let tasks = cw.0.ledger.tasks();
    let count = |s: &str| tasks.iter().filter(|t| t.state == s).count();
    let completed = count("COMPLETED");
    let failed = count("FAILED");
    let cancelled = count("CANCELLED");
    let finished = completed + failed + cancelled;
    let with_result = tasks.iter().filter(|t| t.result.get("verified").is_some()).count();
    let verified = tasks.iter().filter(|t| t.result["verified"] == true).count();
    let n = tasks.len() as f64;
    let ratio = |a: usize, b: usize| (b > 0).then(|| a as f64 / b as f64);
    let mut mins: Vec<f64> = tasks
        .iter()
        .filter(|t| t.state == "COMPLETED")
        .map(|t| (t.updated_at - t.created_at) as f64 / 60_000.0)
        .collect();
    mins.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let tokens: u64 = tasks
        .iter()
        .map(|t| {
            let (i, o) = cw.0.ledger.usage_totals(Some(&t.session));
            i + o
        })
        .sum();
    let interventions: i64 = tasks
        .iter()
        .map(|t| cw.0.ledger.session_interventions(&t.session))
        .sum();
    TaskMetrics {
        total: tasks.len(),
        completed,
        verified,
        failed,
        cancelled,
        active: tasks.len() - finished - count("BLOCKED"),
        completion_rate: ratio(completed, finished),
        verified_rate: ratio(verified, with_result),
        interventions_per_task: (n > 0.0).then(|| interventions as f64 / n),
        tokens_per_task: (n > 0.0).then(|| tokens as f64 / n),
        median_minutes: mins.get(mins.len() / 2).copied(),
    }
}

// ---- Security Center ---------------------------------------------------------

#[derive(Serialize)]
pub struct SecurityStatus {
    #[serde(flatten)]
    pub switches: crate::coworker::ledger::Security,
    pub sandbox_available: bool,
    pub python: Option<String>,
    pub javascript: Option<String>,
}

#[tauri::command]
pub fn coworker_security(cw: State<CoworkerState>) -> SecurityStatus {
    use crate::coworker::sandbox;
    let path = |l: &str| sandbox::interpreter(l).map(|p| p.display().to_string());
    SecurityStatus {
        switches: cw.0.ledger.security(),
        sandbox_available: sandbox::available(),
        python: path("python"),
        javascript: path("javascript"),
    }
}

/// Only the user changes these, from the Security Center.
#[tauri::command]
pub fn coworker_set_security(switches: crate::coworker::ledger::Security, cw: State<CoworkerState>) -> CmdResult<()> {
    let before = cw.0.ledger.security();
    cw.0.ledger.set_security(&switches).map_err(le)?;
    for (name, old, new) in [
        ("paused", before.paused, switches.paused),
        ("network_off", before.network_off, switches.network_off),
        ("code_enabled", before.code_enabled, switches.code_enabled),
        ("auto_web", before.auto_web, switches.auto_web),
    ] {
        if old != new {
            cw.0.ledger
                .record(
                    "",
                    "user",
                    "security",
                    name,
                    &serde_json::Value::Null,
                    if new { "on" } else { "off" },
                    "",
                )
                .map_err(le)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn coworker_standing(cw: State<CoworkerState>) -> Vec<crate::coworker::ledger::StandingRow> {
    cw.0.ledger.standing()
}

#[tauri::command]
pub fn coworker_revoke_standing(
    session: Option<String>,
    tool: Option<String>,
    cw: State<CoworkerState>,
) -> CmdResult<usize> {
    let n =
        cw.0.ledger
            .revoke_standing(session.as_deref(), tool.as_deref())
            .map_err(le)?;
    cw.0.ledger
        .record(
            session.as_deref().unwrap_or(""),
            "user",
            "standing_approval",
            tool.as_deref().unwrap_or("*"),
            &serde_json::Value::Null,
            "revoked",
            &format!("{n} removed"),
        )
        .map_err(le)?;
    Ok(n)
}

#[tauri::command]
pub async fn coworker_stop_all(cw: State<'_, CoworkerState>) -> CmdResult<usize> {
    Ok(cw.0.stop_all().await)
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
