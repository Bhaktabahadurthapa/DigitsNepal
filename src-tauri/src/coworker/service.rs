//! Coworker runtime: the OpenWorker Harness (embedded, in-process) driven by
//! DigitsNepal's policy engine.
//!
//! Every agent runs with `Approvals::Always`, so the engine pauses before each
//! tool call. This service answers each pause with `policy::evaluate`:
//! allowed calls continue automatically, denied calls are refused, and the
//! rest wait for the user's decision in the UI. All outcomes go to the
//! tamper-evident audit log.

use super::agents::{self, AgentDefinition};
use super::ledger::{Ledger, PendingRow, SessionRow};
use super::policy::{self, Decision, ToolHints};
use super::providers::{self, ProviderSpec};
use super::specialists::{self, Specialist};
use super::tools::{self, Translator};
use openworker_sdk::agents::*;
use openworker_sdk::{ApprovalDecision, Harness, Sandbox, Tool};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::oneshot;

/// Receives UI events (`{"session": .., "type": .., ...}`).
pub type Sink = Arc<dyn Fn(Value) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserDecision {
    Once,
    Session,
    Deny,
}

enum Pending {
    Approval {
        tx: oneshot::Sender<UserDecision>,
        session: String,
    },
    Question {
        tx: oneshot::Sender<String>,
        session: String,
    },
}

struct Engine {
    harness: Harness,
    /// Upstream connection name → name actually registered with the engine.
    connections: HashMap<String, String>,
    relays: HashMap<String, super::relay::Relay>,
    agents: HashSet<String>,
}

pub struct Config {
    pub state_dir: PathBuf,
    /// Engine working directory for sessions; must not overlap `state_dir`.
    pub workspace_dir: PathBuf,
    pub backup_dir: PathBuf,
    /// Absolute path of the bundled `openworker-harness` runner, if known.
    pub runner: Option<PathBuf>,
    /// Network endpoints for web and GitHub tools.
    pub web: super::web::WebConfig,
    /// Appended to every tool description. `None` in the app; tests use it to
    /// simulate an update that changes tool definitions.
    pub tool_note: Option<String>,
}

pub struct Coworker {
    pub ledger: Arc<Ledger>,
    cfg: Config,
    sink: Sink,
    engine: tokio::sync::Mutex<Option<Engine>>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    pending: Mutex<HashMap<String, Pending>>,
    hints: RwLock<HashMap<String, ToolHints>>,
    translator: Arc<RwLock<Option<Arc<dyn Translator>>>>,
    /// Turns currently being driven (the engine is only reloaded when idle).
    active: std::sync::atomic::AtomicUsize,
    recovered: std::sync::atomic::AtomicBool,
    /// Sessions with a turn in progress (one run per conversation at a time).
    running: Mutex<HashSet<String>>,
}

#[derive(Debug)]
pub enum TaskSignal {
    ApprovalAsked,
    ApprovalResolved,
    Question,
    Answered,
    ProviderRetry,
    Done(String),
    Cancelled,
    Error(String),
}

struct ActiveTurn<'a>(&'a std::sync::atomic::AtomicUsize);
impl Drop for ActiveTurn<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoworkerError {
    #[error("{0}")]
    Setup(String),
    #[error("{0}")]
    Provider(String),
    #[error("{0}")]
    Engine(String),
    #[error("Unknown session.")]
    UnknownSession,
}

fn engine_err(e: openworker_sdk::Error) -> CoworkerError {
    match e {
        // The request may already have been processed: never retry blindly.
        openworker_sdk::Error::OutcomeUnknown => CoworkerError::Engine(
            "Sahayogi lost contact mid-request, so it may or may not have finished. It was not retried automatically; check the result before asking again.".into(),
        ),
        other => CoworkerError::Engine(other.to_string()),
    }
}

impl Coworker {
    pub fn new(ledger: Arc<Ledger>, cfg: Config, sink: Sink) -> Arc<Self> {
        Arc::new(Self {
            ledger,
            cfg,
            sink,
            engine: tokio::sync::Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            hints: RwLock::new(HashMap::new()),
            translator: Arc::new(RwLock::new(None)),
            active: std::sync::atomic::AtomicUsize::new(0),
            recovered: std::sync::atomic::AtomicBool::new(false),
            running: Mutex::new(HashSet::new()),
        })
    }

    /// Qualified names of the MCP tools the user enabled.
    pub fn enabled_mcp_tools(&self) -> Vec<String> {
        self.ledger
            .mcp_servers()
            .into_iter()
            .flat_map(|s| {
                let name = s.name.clone();
                s.enabled.into_iter().map(move |t| super::mcp::qualified(&name, &t))
            })
            .collect()
    }

    fn refresh_hints(&self) {
        let mut h = HashMap::new();
        for s in self.ledger.mcp_servers() {
            for t in &s.tools {
                h.insert(
                    super::mcp::qualified(&s.name, &t.name),
                    ToolHints {
                        read_only: t.read_only,
                        destructive: t.destructive,
                    },
                );
            }
        }
        *self.hints.write().unwrap() = h;
    }

    /// Add a remote MCP server: connect, list its tools, save it with every
    /// tool disabled. The user then enables tools one by one.
    pub async fn add_mcp_server(&self, name: &str, url: &str) -> Result<super::ledger::McpServerRow, CoworkerError> {
        let name = name.trim().to_ascii_lowercase();
        if !super::mcp::valid_server_name(&name) {
            return Err(CoworkerError::Setup(
                "Use a short name of lowercase letters and digits (e.g. notion).".into(),
            ));
        }
        if self.ledger.mcp_servers().iter().any(|s| s.name == name) {
            return Err(CoworkerError::Setup(
                "A connected app with that name already exists.".into(),
            ));
        }
        let tools = super::mcp::discover(url.trim()).await.map_err(CoworkerError::Setup)?;
        let tools: Vec<_> = tools
            .into_iter()
            .filter(|t| super::mcp::valid_name(&t.name, 50))
            .collect();
        let row = super::ledger::McpServerRow {
            name: name.clone(),
            url: url.trim().to_string(),
            tools,
            enabled: Vec::new(),
            created_at: super::ledger::now(),
        };
        self.ledger
            .save_mcp_server(&row)
            .map_err(|e| CoworkerError::Setup(e.to_string()))?;
        self.ledger
            .record(
                "",
                "user",
                "mcp",
                &name,
                &json!(row.url),
                "added",
                &format!("{} tools", row.tools.len()),
            )
            .ok();
        Ok(row)
    }

    /// Choose which of a connected app's tools Sahayogi may use.
    pub async fn set_mcp_enabled(&self, name: &str, enabled: Vec<String>) -> Result<(), CoworkerError> {
        let mut row = self
            .ledger
            .mcp_servers()
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| CoworkerError::Setup("Unknown connected app.".into()))?;
        row.enabled = enabled
            .into_iter()
            .filter(|t| row.tools.iter().any(|x| &x.name == t))
            .collect();
        self.reload_tools().await?;
        self.ledger
            .save_mcp_server(&row)
            .map_err(|e| CoworkerError::Setup(e.to_string()))?;
        self.ledger
            .record("", "user", "mcp", name, &json!(row.enabled), "tools_enabled", "")
            .ok();
        self.refresh_hints();
        Ok(())
    }

    pub async fn remove_mcp_server(&self, name: &str) -> Result<(), CoworkerError> {
        self.reload_tools().await?;
        self.ledger
            .delete_mcp_server(name)
            .map_err(|e| CoworkerError::Setup(e.to_string()))?;
        self.ledger
            .record("", "user", "mcp", name, &Value::Null, "removed", "")
            .ok();
        self.refresh_hints();
        Ok(())
    }

    /// Restart the engine so tool changes (connected apps) take effect.
    /// Refused while a chat is running or waiting for the user.
    pub async fn reload_tools(&self) -> Result<(), CoworkerError> {
        if self.active.load(std::sync::atomic::Ordering::SeqCst) > 0 || !self.pending.lock().unwrap().is_empty() {
            return Err(CoworkerError::Setup(
                "Finish or stop the running chat first, then try again.".into(),
            ));
        }
        self.shutdown().await;
        self.sessions.lock().unwrap().clear();
        self.refresh_hints();
        Ok(())
    }

    /// Give agents access to DigitsNepal's translation engine.
    pub fn set_translator(&self, t: Arc<dyn Translator>) {
        *self.translator.write().unwrap() = Some(t);
    }

    /// Approvals and questions still waiting for the user (survive restarts).
    pub fn pending(&self) -> Vec<PendingRow> {
        self.ledger.unresolved_pending().unwrap_or_default()
    }

    fn emit(&self, session: &str, kind: &str, mut payload: Value) {
        let signal = match kind {
            "approval_required" => Some(TaskSignal::ApprovalAsked),
            "approval_resolved" => Some(TaskSignal::ApprovalResolved),
            "question" => Some(TaskSignal::Question),
            "provider_error" if payload["retryable"] == true => Some(TaskSignal::ProviderRetry),
            "done" => Some(TaskSignal::Done(payload["text"].as_str().unwrap_or("").to_string())),
            "cancelled" => Some(TaskSignal::Cancelled),
            "error" => Some(TaskSignal::Error(payload["message"].as_str().unwrap_or("").to_string())),
            _ => None,
        };
        if matches!(kind, "done" | "cancelled" | "error") {
            // The turn is over: the conversation accepts the next message.
            self.running.lock().unwrap().remove(session);
        }
        payload["session"] = json!(session);
        payload["type"] = json!(kind);
        (self.sink)(payload);
        if let Some(sig) = signal {
            self.task_event(session, sig);
        }
    }

    /// Keep a task's state in step with what the engine is doing.
    fn task_event(&self, session: &str, signal: TaskSignal) {
        use super::tasks::TaskState::*;
        let Some(t) = self.ledger.task_for_session(session) else {
            return;
        };
        let Some(state) = super::tasks::TaskState::parse(&t.state) else {
            return;
        };
        let (to, why) = match signal {
            TaskSignal::ApprovalAsked => (WaitingForApproval, "Waiting for your approval".to_string()),
            // Only a task that was waiting on this, and is not paused, resumes.
            TaskSignal::ApprovalResolved if state == WaitingForApproval && !t.paused => {
                (Running, "Continuing".to_string())
            }
            TaskSignal::Answered if state == NeedsClarification && !t.paused => (Running, "Continuing".to_string()),
            TaskSignal::ApprovalResolved | TaskSignal::Answered => return,
            TaskSignal::Question => (NeedsClarification, "Waiting for your answer".to_string()),
            TaskSignal::ProviderRetry => (Retrying, "The AI provider had a temporary problem".to_string()),
            TaskSignal::Done(text) => {
                if matches!(state, Completed | Failed | Blocked | Cancelled) {
                    return;
                }
                if t.paused {
                    (Blocked, "Paused by you".to_string())
                } else if text.trim_end().ends_with('?') {
                    (
                        NeedsClarification,
                        "Sahayogi asked you something in the conversation".to_string(),
                    )
                } else {
                    (
                        Blocked,
                        "Sahayogi stopped before finishing. Reply in the conversation or press Resume.".to_string(),
                    )
                }
            }
            TaskSignal::Cancelled if t.paused => (Blocked, "Paused by you".to_string()),
            TaskSignal::Cancelled => (Cancelled, "Cancelled".to_string()),
            TaskSignal::Error(m) => (Failed, m),
        };
        if super::tasks::transition(&self.ledger, &t, to, &why) {
            (self.sink)(json!({"type": "task", "session": session, "task": t.id, "state": to.as_str(), "reason": why}));
        }
    }

    // ---- Tasks ----------------------------------------------------------------

    /// Create a task from a request in any language and start it.
    pub async fn create_task(
        self: &Arc<Self>,
        spec: &ProviderSpec,
        request: &str,
        specialist: Option<&str>,
        auto_web: bool,
    ) -> Result<super::ledger::TaskRow, CoworkerError> {
        let request = request.trim();
        if request.is_empty() {
            return Err(CoworkerError::Setup("Describe what you need.".into()));
        }
        let hints = super::intent::analyze(request);
        let sp = specialist
            .or_else(|| super::tasks::playbook(&hints.category).map(|p| p.specialist))
            .unwrap_or("general");
        let row = self.new_session_as(spec, sp).await?;
        let now = super::ledger::now();
        let task = super::ledger::TaskRow {
            id: format!("task_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]),
            session: row.id.clone(),
            specialist: sp.to_string(),
            request: request.to_string(),
            intent: serde_json::to_value(&hints).unwrap_or(Value::Null),
            plan: json!({}),
            evidence: json!([]),
            result: json!({}),
            state: "CREATED".into(),
            state_reason: "Created".into(),
            paused: false,
            created_at: now,
            updated_at: now,
        };
        self.ledger
            .insert_task(&task)
            .map_err(|e| CoworkerError::Setup(e.to_string()))?;
        self.ledger
            .upsert_session(&SessionRow {
                title: request.chars().take(60).collect(),
                ..row
            })
            .ok();
        self.ledger
            .record(&task.session, "user", "task", "", &json!(request), "created", &task.id)
            .ok();
        if auto_web {
            // "Research the web without asking" for this task only: standing,
            // revocable permissions for the read-only web tools.
            for t in super::tools::READ_ONLY_WEB {
                self.ledger.add_standing(&task.session, t).ok();
            }
            self.ledger
                .record(
                    &task.session,
                    "user",
                    "standing_approval",
                    "web research",
                    &Value::Null,
                    "granted",
                    "for this task",
                )
                .ok();
        }
        super::tasks::transition(&self.ledger, &task, super::tasks::TaskState::Planning, "Planning");
        self.send(&task.session, &super::tasks::start_message(&task, &hints), spec)
            .await?;
        Ok(self.ledger.task(&task.id).unwrap_or(task))
    }

    fn task_or_err(&self, id: &str) -> Result<super::ledger::TaskRow, CoworkerError> {
        self.ledger
            .task(id)
            .ok_or_else(|| CoworkerError::Setup("Unknown task.".into()))
    }

    /// Pause: stop the current step; the task waits until resumed.
    pub async fn pause_task(&self, id: &str) -> Result<(), CoworkerError> {
        let t = self.task_or_err(id)?;
        self.ledger.set_task_paused(id, true).ok();
        super::tasks::transition(&self.ledger, &t, super::tasks::TaskState::Blocked, "Paused by you");
        self.cancel(&t.session).await;
        Ok(())
    }

    /// Resume a paused, blocked or interrupted task.
    pub async fn resume_task(self: &Arc<Self>, id: &str, spec: &ProviderSpec) -> Result<(), CoworkerError> {
        let t = self.task_or_err(id)?;
        if self.waiting_for_user(&t.session) {
            return Err(CoworkerError::Setup(
                "This task is waiting for you: answer it under \"Needs you\".".into(),
            ));
        }
        if self.running.lock().unwrap().contains(&t.session) {
            return Err(CoworkerError::Setup("This task is still working.".into()));
        }
        self.ledger.set_task_paused(id, false).ok();
        if self.resume_pending(&t.session, spec).await? {
            super::tasks::transition(&self.ledger, &t, super::tasks::TaskState::Running, "Resumed");
            return Ok(());
        }
        self.send(
            &t.session,
            &format!("[Task {} resume] Continue this task from where you stopped. Check which plan steps are done (do not repeat them), then finish with task.complete.", t.id),
            spec,
        )
        .await
    }

    pub async fn cancel_task(&self, id: &str) -> Result<(), CoworkerError> {
        let t = self.task_or_err(id)?;
        self.ledger.set_task_paused(id, false).ok();
        super::tasks::transition(&self.ledger, &t, super::tasks::TaskState::Cancelled, "Cancelled by you");
        self.cancel(&t.session).await;
        Ok(())
    }

    pub async fn retry_task(self: &Arc<Self>, id: &str, spec: &ProviderSpec) -> Result<(), CoworkerError> {
        let t = self.task_or_err(id)?;
        if !super::tasks::transition(&self.ledger, &t, super::tasks::TaskState::Retrying, "Retrying") {
            return Err(CoworkerError::Setup(
                "Only a failed task can be retried; use Resume instead.".into(),
            ));
        }
        self.send(
            &t.session,
            &format!("[Task {} retry] The last attempt failed ({}). Continue from the last completed step, then finish with task.complete.", t.id, t.state_reason),
            spec,
        )
        .await
    }

    /// After a restart nothing is running: mark interrupted tasks so the user
    /// can resume them. Runs once per launch.
    pub fn recover_tasks(&self) {
        use super::tasks::TaskState::*;
        if self.recovered.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        for t in self.ledger.tasks() {
            if matches!(
                super::tasks::TaskState::parse(&t.state),
                Some(Planning | Ready | Running | Retrying | WaitingForApproval | WaitingForExternalResponse)
            ) {
                super::tasks::transition(
                    &self.ledger,
                    &t,
                    Blocked,
                    "DigitsNepal was closed while this task was running. Press Resume to continue.",
                );
            }
        }
    }

    // ---- Engine lifecycle ---------------------------------------------------

    async fn ensure_engine(&self) -> Result<tokio::sync::MutexGuard<'_, Option<Engine>>, CoworkerError> {
        let mut guard = self.engine.lock().await;
        if guard.is_none() {
            let harness = match self.start_engine().await {
                // The engine never lets a saved tool definition change. After an
                // app update changed a tool, start from fresh engine state.
                Err(e) if e.to_string().contains("tool definition changed") => {
                    self.replace_engine_state()?;
                    self.start_engine().await?
                }
                other => other?,
            };
            *guard = Some(Engine {
                harness,
                connections: HashMap::new(),
                relays: HashMap::new(),
                agents: HashSet::new(),
            });
        }
        Ok(guard)
    }

    async fn start_engine(&self) -> Result<Harness, CoworkerError> {
        std::fs::create_dir_all(&self.cfg.state_dir).map_err(|e| CoworkerError::Setup(e.to_string()))?;
        let mut b = Harness::builder()
            .state_dir(self.cfg.state_dir.clone())
            .sandbox(Sandbox::Off)
            .persist_sessions(true)
            // Summarise older history before the context window fills.
            .compact_at_context_fraction(0.8);
        if let Some(r) = &self.cfg.runner {
            b = b.executable(r.clone());
        }
        let harness = b
            .start()
            .await
            .map_err(|e| CoworkerError::Setup(format!("Sahayogi could not start: {e}")))?;
        self.refresh_hints();
        if let Err(e) = register_tools(
            &harness,
            self.ledger.clone(),
            self.cfg.backup_dir.clone(),
            self.translator.clone(),
            self.cfg.web.clone(),
            self.cfg.tool_note.clone(),
        )
        .await
        {
            let _ = harness.shutdown().await;
            return Err(e);
        }
        Ok(harness)
    }

    /// Move the engine's state aside (keeping the most recent copy) and mark
    /// everything that depended on it. Messages, tasks and the audit log live
    /// in DigitsNepal's own ledger and are kept.
    fn replace_engine_state(&self) -> Result<(), CoworkerError> {
        let dir = &self.cfg.state_dir;
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("engine").to_string();
        let parent = dir.parent().unwrap_or(dir);
        if let Ok(rd) = std::fs::read_dir(parent) {
            for e in rd.flatten() {
                if e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{name}-replaced-"))
                {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        let backup = parent.join(format!("{name}-replaced-{}", super::ledger::now()));
        if dir.exists() {
            std::fs::rename(dir, &backup)
                .map_err(|e| CoworkerError::Setup(format!("Could not update Sahayogi's engine data: {e}")))?;
        }
        let at = super::ledger::now();
        self.ledger.kv_set("engine_reset_at", &at.to_string()).ok();
        self.sessions.lock().unwrap().clear();
        for p in self.ledger.unresolved_pending().unwrap_or_default() {
            self.ledger.resolve_pending(&p.id, "expired").ok();
        }
        let why = "DigitsNepal was updated and this task can't continue. Its plan and results are kept; start it again as a new task.";
        for t in self.ledger.tasks() {
            if !super::tasks::transition(&self.ledger, &t, super::tasks::TaskState::Blocked, why)
                && t.state == "BLOCKED"
            {
                // Already waiting: update the reason so the user sees why.
                self.ledger.set_task_state(&t.id, "BLOCKED", "BLOCKED", why).ok();
            }
        }
        self.ledger
            .record(
                "",
                "system",
                "engine",
                "",
                &Value::Null,
                "replaced",
                "Tool definitions changed after an update; engine state started fresh.",
            )
            .ok();
        Ok(())
    }

    /// Conversations created before the engine state was replaced cannot be
    /// continued (their engine sessions are gone).
    fn predates_reset(&self, created_at: i64) -> bool {
        self.ledger
            .kv_get("engine_reset_at")
            .and_then(|v| v.parse::<i64>().ok())
            .is_some_and(|at| created_at < at)
    }

    pub async fn shutdown(&self) {
        if let Some(e) = self.engine.lock().await.take() {
            let _ = e.harness.shutdown().await;
        }
    }

    /// Register (once) the model connection; returns the engine's name for it.
    async fn ensure_connection(&self, engine: &mut Engine, spec: &ProviderSpec) -> Result<String, CoworkerError> {
        if let Some(c) = engine.connections.get(&spec.connection) {
            return Ok(c.clone());
        }
        let engine_spec = self.engine_spec(engine, spec).await?;
        let mut name = engine_spec.connection.clone();
        if let Err(e) = engine
            .harness
            .set_model_provider(providers::to_engine(&engine_spec))
            .await
        {
            // A moved relay port changes the endpoint, which the engine
            // treats as a new connection identity.
            let port = engine_spec
                .base_url
                .rsplit(':')
                .next()
                .unwrap_or("")
                .split('/')
                .next()
                .unwrap_or("")
                .to_string();
            name = format!("{}-p{port}", engine_spec.connection);
            let retry = ProviderSpec {
                connection: name.clone(),
                ..engine_spec
            };
            engine
                .harness
                .set_model_provider(providers::to_engine(&retry))
                .await
                .map_err(|_| CoworkerError::Provider(format!("Could not connect the AI provider: {e}")))?;
        }
        engine.connections.insert(spec.connection.clone(), name.clone());
        Ok(name)
    }

    /// Store (once, immutably) and register an agent definition.
    async fn register_definition(&self, engine: &mut Engine, def: &AgentDefinition) -> Result<String, CoworkerError> {
        let id = def.id();
        self.ledger
            .save_definition(&id, &def.key, def.version, &def.to_json())
            .map_err(|e| CoworkerError::Setup(e.to_string()))?;
        if !engine.agents.contains(&id) {
            engine
                .harness
                .create_agent(agents::to_engine(def, &id))
                .await
                .map_err(engine_err)?;
            engine.agents.insert(id.clone());
        }
        Ok(id)
    }

    /// The current definition of a specialist for this provider; returns its ID.
    async fn prepare_agent(&self, spec: &ProviderSpec, specialist: &Specialist) -> Result<String, CoworkerError> {
        let mut guard = self.ensure_engine().await?;
        let engine = guard.as_mut().expect("engine started");
        let connection = self.ensure_connection(engine, spec).await?;
        let mut def = agents::for_specialist(specialist, &connection, &spec.model);
        if specialist.key == "general" {
            // Connected apps the user enabled are available to the general coworker.
            def.tools.extend(self.enabled_mcp_tools());
        }
        self.register_definition(engine, &def).await
    }

    /// Cloud OpenAI-compatible providers go through the local compatibility
    /// relay (which also keeps the real key out of the engine). Gemini and
    /// local servers connect directly.
    async fn engine_spec(&self, engine: &mut Engine, spec: &ProviderSpec) -> Result<ProviderSpec, CoworkerError> {
        let needs_relay = spec.api == providers::Api::ChatCompletions
            && spec.cloud
            && !spec.base_url.contains("generativelanguage.googleapis.com");
        if !needs_relay {
            return Ok(spec.clone());
        }
        if !engine.relays.contains_key(&spec.connection) {
            let key = format!("relay_port:{}", spec.connection);
            let port: u16 = self.ledger.kv_get(&key).and_then(|p| p.parse().ok()).unwrap_or(0);
            let relay = super::relay::start(port, &spec.base_url, spec.key.clone())
                .await
                .map_err(|e| CoworkerError::Setup(format!("Could not start the provider relay: {e}")))?;
            if let Some(p) = relay.base_url.rsplit(':').next().and_then(|x| x.split('/').next()) {
                self.ledger.kv_set(&key, p).ok();
            }
            engine.relays.insert(spec.connection.clone(), relay);
        }
        let r = &engine.relays[&spec.connection];
        Ok(ProviderSpec {
            base_url: r.base_url.clone(),
            key: Some(r.token.clone()),
            ..spec.clone()
        })
    }

    // ---- Sessions -----------------------------------------------------------

    /// Start a conversation with the general coworker.
    pub async fn new_session(&self, spec: &ProviderSpec) -> Result<SessionRow, CoworkerError> {
        self.new_session_as(spec, "general").await
    }

    /// Start a conversation with a specialist (`general`, `writer`, ...).
    pub async fn new_session_as(&self, spec: &ProviderSpec, specialist: &str) -> Result<SessionRow, CoworkerError> {
        let sp = specialists::find(specialist)
            .ok_or_else(|| CoworkerError::Setup(format!("Unknown specialist: {specialist}")))?;
        let agent = self.prepare_agent(spec, sp).await?;
        let guard = self.engine.lock().await;
        let harness = &guard.as_ref().expect("engine started").harness;
        std::fs::create_dir_all(&self.cfg.workspace_dir).map_err(|e| CoworkerError::Setup(e.to_string()))?;
        let session = harness
            .session(&agent, &self.cfg.workspace_dir)
            .await
            .map_err(engine_err)?;
        let now = super::ledger::now();
        let row = SessionRow {
            id: session.id().to_string(),
            agent,
            specialist: sp.key.into(),
            title: "New conversation".into(),
            created_at: now,
            updated_at: now,
        };
        self.ledger
            .upsert_session(&row)
            .map_err(|e| CoworkerError::Setup(e.to_string()))?;
        self.sessions.lock().unwrap().insert(row.id.clone(), Arc::new(session));
        Ok(row)
    }

    /// Reattach a session saved by an earlier launch.
    async fn session(&self, id: &str, spec: &ProviderSpec) -> Result<Arc<Session>, CoworkerError> {
        if let Some(s) = self.sessions.lock().unwrap().get(id) {
            return Ok(s.clone());
        }
        let row = self
            .ledger
            .sessions()
            .map_err(|e| CoworkerError::Setup(e.to_string()))?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or(CoworkerError::UnknownSession)?;
        if self.predates_reset(row.created_at) {
            return Err(CoworkerError::Setup(
                "This conversation was started before DigitsNepal was updated and can't continue. Its messages are kept; start a new conversation.".into(),
            ));
        }
        {
            let mut guard = self.ensure_engine().await?;
            let engine = guard.as_mut().expect("engine started");
            let connection = self.ensure_connection(engine, spec).await?;
            // Resume with the exact definition the session started with.
            let def = self
                .ledger
                .definition(&row.agent)
                .and_then(|v| AgentDefinition::from_json(&v))
                .ok_or_else(|| {
                    CoworkerError::Setup(
                        "This conversation was created by an older version of Sahayogi. Start a new conversation."
                            .into(),
                    )
                })?;
            if def.connection != connection {
                return Err(CoworkerError::Setup(
                    "The AI provider changed since this conversation started. Start a new conversation.".into(),
                ));
            }
            self.register_definition(engine, &def).await?;
        }
        let guard = self.engine.lock().await;
        let harness = &guard.as_ref().expect("engine started").harness;
        let s = harness
            .resume(SessionRef {
                session_id: row.id.clone(),
                agent: row.agent.clone(),
                team: None,
            })
            .await
            .map_err(engine_err)?;
        let s = Arc::new(s);
        self.sessions.lock().unwrap().insert(row.id, s.clone());
        Ok(s)
    }

    /// Send a message and drive the turn to completion in the background,
    /// streaming events to the UI and routing every tool call through policy.
    pub async fn send(
        self: &Arc<Self>,
        session_id: &str,
        text: &str,
        spec: &ProviderSpec,
    ) -> Result<(), CoworkerError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        if self.waiting_for_user(session_id) {
            return Err(CoworkerError::Setup(
                "Sahayogi is waiting for you in this conversation. Answer its question or approval first.".into(),
            ));
        }
        if !self.running.lock().unwrap().insert(session_id.to_string()) {
            return Err(CoworkerError::Setup(
                "Sahayogi is still working on this. Wait for it to finish, or press Stop.".into(),
            ));
        }
        let session = match self.session(session_id, spec).await {
            Ok(s) => s,
            Err(e) => {
                self.running.lock().unwrap().remove(session_id);
                return Err(e);
            }
        };
        self.ledger.add_message(session_id, "user", text).ok();
        if let Some(mut row) = self
            .ledger
            .sessions()
            .ok()
            .and_then(|v| v.into_iter().find(|r| r.id == session_id))
        {
            if row.title == "New conversation" {
                row.title = text.chars().take(60).collect();
            }
            row.updated_at = super::ledger::now();
            self.ledger.upsert_session(&row).ok();
        }
        self.ledger
            .record(session_id, "user", "message", "", &json!(text), "sent", "")
            .ok();
        if let Some(t) = self.ledger.task_for_session(session_id) {
            use super::tasks::TaskState::*;
            if matches!(
                super::tasks::TaskState::parse(&t.state),
                Some(Blocked | NeedsClarification | Retrying)
            ) {
                self.ledger.set_task_paused(&t.id, false).ok();
                super::tasks::transition(&self.ledger, &t, Running, "Continuing");
            }
        }
        let me = self.clone();
        let sid = session_id.to_string();
        let input = text.to_string();
        tokio::spawn(async move {
            me.emit(&sid, "status", json!({"state": "running"}));
            match me.drive(&sid, &session, Some(&input)).await {
                Ok(()) => {}
                Err(e) => {
                    me.emit(&sid, "error", json!({"message": e.to_string()}));
                    me.ledger
                        .record(&sid, "system", "error", "", &Value::Null, "failed", &e.to_string())
                        .ok();
                }
            }
            me.running.lock().unwrap().remove(&sid);
            me.emit(&sid, "status", json!({"state": "idle"}));
        });
        Ok(())
    }

    /// Continue a session whose turn paused for approval before a restart.
    /// Answered calls are not re-run; open approvals are shown again.
    pub async fn resume_pending(
        self: &Arc<Self>,
        session_id: &str,
        spec: &ProviderSpec,
    ) -> Result<bool, CoworkerError> {
        if !self.pending().iter().any(|p| p.session == session_id) {
            return Ok(false);
        }
        if self.pending.lock().unwrap().values().any(|p| match p {
            Pending::Approval { session, .. } | Pending::Question { session, .. } => session == session_id,
        }) {
            return Ok(false); // already waiting in this process
        }
        if !self.running.lock().unwrap().insert(session_id.to_string()) {
            return Ok(false);
        }
        let session = match self.session(session_id, spec).await {
            Ok(s) => s,
            Err(e) => {
                self.running.lock().unwrap().remove(session_id);
                return Err(e);
            }
        };
        let me = self.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            me.emit(&sid, "status", json!({"state": "running"}));
            if let Err(e) = me.drive(&sid, &session, None).await {
                me.emit(&sid, "error", json!({"message": e.to_string()}));
            }
            me.running.lock().unwrap().remove(&sid);
            me.emit(&sid, "status", json!({"state": "idle"}));
        });
        Ok(true)
    }

    async fn drive(&self, sid: &str, session: &Session, input: Option<&str>) -> Result<(), CoworkerError> {
        self.active.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _turn = ActiveTurn(&self.active);
        let mut stream = match input {
            Some(text) => session.run_streamed(text).await,
            None => session.resume_streamed().await,
        }
        .map_err(engine_err)?;
        loop {
            while let Some(ev) = stream.next().await {
                self.forward(sid, ev);
            }
            let result = stream.finish().await.map_err(engine_err)?;
            match result.status {
                RunStatus::Completed => {
                    let text = result.final_output.unwrap_or_default();
                    self.ledger.add_message(sid, "assistant", &text).ok();
                    self.emit(sid, "done", json!({"text": text}));
                    return Ok(());
                }
                RunStatus::Cancelled => {
                    self.emit(sid, "cancelled", json!({}));
                    return Ok(());
                }
                RunStatus::Failed(why) => {
                    let detail = result
                        .provider_error
                        .map(|d| {
                            format!(
                                "{:?}{}{}",
                                d.category,
                                d.code.map(|c| format!(" [{c}]")).unwrap_or_default(),
                                d.http_status.map(|s| format!(" (HTTP {s})")).unwrap_or_default()
                            )
                        })
                        .unwrap_or(why);
                    return Err(CoworkerError::Provider(format!("The model request failed: {detail}")));
                }
                RunStatus::Interrupted => {
                    if result.interruptions.is_empty() {
                        return Err(CoworkerError::Engine(
                            "The turn paused without a pending request.".into(),
                        ));
                    }
                    for i in &result.interruptions {
                        self.resolve(sid, session, i).await?;
                    }
                    stream = session.resume_streamed().await.map_err(engine_err)?;
                }
            }
        }
    }

    fn forward(&self, sid: &str, ev: RunEvent) {
        match ev {
            RunEvent::AttemptStarted { attempt, .. } if attempt > 1 => self.emit(sid, "reset", json!({})),
            RunEvent::TextDelta { text, .. } => self.emit(sid, "text", json!({"text": text})),
            RunEvent::ToolStarted { tool, qualified_name, arguments, call_id, .. } => self.emit(
                sid,
                "tool_started",
                json!({"tool": qualified_name.unwrap_or(tool), "call_id": call_id, "arguments": preview(arguments.as_ref())}),
            ),
            RunEvent::ToolFinished { tool, qualified_name, call_id, .. } => {
                self.emit(sid, "tool_finished", json!({"tool": qualified_name.unwrap_or(tool), "call_id": call_id}))
            }
            RunEvent::ProviderError { diagnostic, .. } => self.emit(
                sid,
                "provider_error",
                json!({"category": format!("{:?}", diagnostic.category), "retryable": diagnostic.retryable}),
            ),
            // Token usage for each model call (input, cached input, output).
            RunEvent::Other(e) if e.kind == "model_usage" => {
                let input = e.data["input_tokens"].as_u64().unwrap_or(0);
                let output = e.data["output_tokens"].as_u64().unwrap_or(0);
                if input + output > 0 {
                    self.ledger.add_usage(sid, "", input, output).ok();
                    let (ti, to) = self.ledger.usage_totals(Some(sid));
                    self.emit(
                        sid,
                        "usage",
                        json!({"input_tokens": input, "output_tokens": output, "session_input": ti, "session_output": to}),
                    );
                }
            }
            _ => {}
        }
    }

    async fn resolve(&self, sid: &str, session: &Session, i: &Interruption) -> Result<(), CoworkerError> {
        match &i.kind {
            InterruptionKind::Approval {
                tool,
                qualified_name,
                arguments,
                ..
            } => {
                let q = qualified_name.clone().unwrap_or_else(|| tool.replacen("__", ".", 1));
                let roots = self.ledger.granted_roots();
                let hints = self.hints.read().unwrap().clone();
                let ledger = self.ledger.clone();
                let security = self.ledger.security();
                let lookup = move |name: &str| tools::automation_steps(&ledger, name);
                let decision = policy::evaluate(
                    &q,
                    arguments,
                    &policy::Context {
                        roots: &roots,
                        hints: &hints,
                        automation: &lookup,
                        security: &security,
                    },
                );
                let response = match &decision {
                    Decision::Allow { reason } => {
                        self.ledger
                            .record(sid, "policy", "tool_call", &q, arguments, "allow", reason)
                            .ok();
                        Response::Approve(ApprovalDecision::Once)
                    }
                    Decision::Deny { reason } => {
                        self.ledger
                            .record(sid, "policy", "tool_call", &q, arguments, "deny", reason)
                            .ok();
                        self.emit(sid, "denied", json!({"tool": q, "reason": reason}));
                        Response::Deny
                    }
                    // "Allow for this chat" is kept by DigitsNepal (revocable in
                    // the Security Center), not by the engine.
                    Decision::Ask { standing_ok: true, .. } if self.ledger.has_standing(sid, &q) => {
                        self.ledger
                            .record(
                                sid,
                                "policy",
                                "tool_call",
                                &q,
                                arguments,
                                "allow",
                                "You allowed this for this chat.",
                            )
                            .ok();
                        Response::Approve(ApprovalDecision::Once)
                    }
                    Decision::Ask {
                        reason,
                        standing_ok,
                        hard_floor,
                    } => {
                        self.ledger
                            .record(sid, "policy", "tool_call", &q, arguments, "ask", reason)
                            .ok();
                        self.ledger
                            .add_pending(
                                &i.id,
                                sid,
                                "approval",
                                &q,
                                &preview(Some(arguments)),
                                reason,
                                *standing_ok,
                                *hard_floor,
                            )
                            .ok();
                        let (tx, rx) = oneshot::channel();
                        self.pending.lock().unwrap().insert(
                            i.id.clone(),
                            Pending::Approval {
                                tx,
                                session: sid.to_string(),
                            },
                        );
                        self.emit(
                            sid,
                            "approval_required",
                            json!({"id": i.id, "tool": q, "arguments": preview(Some(arguments)), "reason": reason,
                                   "standing_ok": standing_ok, "hard_floor": hard_floor}),
                        );
                        // A dropped sender (cancel, shutdown) counts as a denial.
                        let user = rx.await.unwrap_or(UserDecision::Deny);
                        let effective = match user {
                            UserDecision::Session if !standing_ok => UserDecision::Once,
                            d => d,
                        };
                        self.ledger
                            .record(
                                sid,
                                "user",
                                "approval",
                                &q,
                                arguments,
                                &format!("{effective:?}").to_lowercase(),
                                "",
                            )
                            .ok();
                        self.ledger
                            .resolve_pending(&i.id, &format!("{effective:?}").to_lowercase())
                            .ok();
                        self.emit(sid, "approval_resolved", json!({"id": i.id, "decision": effective}));
                        match effective {
                            UserDecision::Once => Response::Approve(ApprovalDecision::Once),
                            UserDecision::Session => {
                                self.ledger.add_standing(sid, &q).ok();
                                // The engine approves only this call; later calls come
                                // back through policy, so revoking takes effect at once.
                                Response::Approve(ApprovalDecision::Once)
                            }
                            UserDecision::Deny => Response::Deny,
                        }
                    }
                };
                session.respond(i, response).await.map_err(engine_err)
            }
            InterruptionKind::Question { question, options } => {
                self.ledger
                    .add_pending(
                        &i.id,
                        sid,
                        "question",
                        "",
                        &json!({"question": question, "options": options}),
                        "",
                        false,
                        false,
                    )
                    .ok();
                let (tx, rx) = oneshot::channel();
                self.pending.lock().unwrap().insert(
                    i.id.clone(),
                    Pending::Question {
                        tx,
                        session: sid.to_string(),
                    },
                );
                self.emit(
                    sid,
                    "question",
                    json!({"id": i.id, "question": question, "options": options}),
                );
                let answer = rx.await.unwrap_or_default();
                self.ledger.resolve_pending(&i.id, "answered").ok();
                self.ledger
                    .record(sid, "user", "answer", "", &json!(answer), "answered", "")
                    .ok();
                self.task_event(sid, TaskSignal::Answered);
                session.respond(i, Response::Answer(answer)).await.map_err(engine_err)
            }
            other => Err(CoworkerError::Engine(format!(
                "The engine paused for a safety check: {other:?}"
            ))),
        }
    }

    pub fn decide(&self, id: &str, decision: UserDecision) -> bool {
        // Take the entry with the lock held only for this statement.
        let entry = {
            let mut map = self.pending.lock().unwrap();
            match map.get(id) {
                Some(Pending::Approval { .. }) => map.remove(id),
                _ => None,
            }
        };
        match entry {
            Some(Pending::Approval { tx, .. }) => tx.send(decision).is_ok(),
            _ => false,
        }
    }

    pub fn answer(&self, id: &str, text: &str) -> bool {
        let entry = {
            let mut map = self.pending.lock().unwrap();
            match map.get(id) {
                Some(Pending::Question { .. }) => map.remove(id),
                _ => None,
            }
        };
        match entry {
            Some(Pending::Question { tx, .. }) => tx.send(text.to_string()).is_ok(),
            _ => false,
        }
    }

    /// Stop the session's turn. Pending approvals are denied, never approved.
    pub async fn cancel(&self, session_id: &str) {
        self.pending.lock().unwrap().retain(|_, p| match p {
            Pending::Approval { session, .. } | Pending::Question { session, .. } => session != session_id,
        });
        for p in self.pending().into_iter().filter(|p| p.session == session_id) {
            self.ledger.resolve_pending(&p.id, "cancelled").ok();
        }
        let s = self.sessions.lock().unwrap().get(session_id).cloned();
        if let Some(s) = s {
            let _ = s.cancel().await;
        }
        self.ledger
            .record(session_id, "user", "cancel", "", &Value::Null, "cancelled", "")
            .ok();
    }

    /// Emergency stop: cancel every running or waiting chat.
    pub async fn stop_all(&self) -> usize {
        let mut ids: HashSet<String> = self.sessions.lock().unwrap().keys().cloned().collect();
        ids.extend(self.pending().into_iter().map(|p| p.session));
        for id in &ids {
            self.cancel(id).await;
        }
        ids.len()
    }

    /// An approval or question in this conversation is waiting in this launch.
    fn waiting_for_user(&self, session_id: &str) -> bool {
        self.pending.lock().unwrap().values().any(|p| match p {
            Pending::Approval { session, .. } | Pending::Question { session, .. } => session == session_id,
        })
    }

    /// Whether an open approval can be answered right now (its chat is
    /// waiting in this launch) rather than after reopening the chat.
    pub fn is_live(&self, id: &str) -> bool {
        self.pending.lock().unwrap().contains_key(id)
    }
}

fn preview(v: Option<&Value>) -> Value {
    match v {
        // Code is always shown in full (capped at 20,000 characters by its
        // schema): the user must see exactly what they approve.
        Some(v) if v.get("code").is_some_and(Value::is_string) => v.clone(),
        Some(v) => {
            let s = v.to_string();
            if s.len() <= 4000 {
                v.clone()
            } else {
                json!({"preview": s.chars().take(4000).collect::<String>(), "truncated": true})
            }
        }
        None => Value::Null,
    }
}

/// Register the typed tool catalogue (and legacy aliases) with the engine.
/// Every call runs through `tools::run_guarded`: timeout, cancellation,
/// audit, and the handler's own folder re-check.
async fn register_tools(
    h: &Harness,
    ledger: Arc<Ledger>,
    backups: PathBuf,
    translator: Arc<RwLock<Option<Arc<dyn Translator>>>>,
    web: super::web::WebConfig,
    tool_note: Option<String>,
) -> Result<(), CoworkerError> {
    let mut by_ns: std::collections::BTreeMap<String, Vec<Tool>> = Default::default();
    let mut add = |qualified: &str, spec: &tools::ToolSpec| {
        let (ns, short) = qualified.split_once('.').expect("qualified tool name");
        let name = qualified.to_string();
        let ledger = ledger.clone();
        let backups = backups.clone();
        let translator = translator.clone();
        let web = web.clone();
        let description = match &tool_note {
            Some(n) => format!("{} {n}", spec.description),
            None => spec.description.to_string(),
        };
        let tool = Tool::new(short)
            .description(description)
            .input_schema(spec.input_schema.clone())
            .handler(move |args: Value, ctx| {
                let deps = tools::Deps {
                    ledger: ledger.clone(),
                    backups: backups.clone(),
                    translator: translator.read().unwrap().clone(),
                    web: web.clone(),
                    sandbox_dir: backups.with_file_name("sandbox"),
                    session: ctx.session_id.clone(),
                };
                let name = name.clone();
                async move { Ok(tools::run_guarded(&name, args, &deps, &ctx.session_id, ctx.cancellation()).await) }
            });
        by_ns.entry(ns.to_string()).or_default().push(tool);
    };
    for spec in tools::specs() {
        add(spec.name, &spec);
    }
    for (old, _) in tools::LEGACY {
        if let Some(spec) = tools::spec(old) {
            add(old, &spec);
        }
    }
    // Tools of connected MCP apps the user enabled.
    for server in ledger.mcp_servers() {
        for t in server.tools.iter().filter(|t| server.enabled.contains(&t.name)) {
            let (url, remote, qualified) = (
                server.url.clone(),
                t.name.clone(),
                super::mcp::qualified(&server.name, &t.name),
            );
            let ledger = ledger.clone();
            let tool = Tool::new(super::mcp::short_name(&server.name, &t.name))
                .description(format!("[Connected app: {}] {}", server.name, t.description))
                .input_schema(t.input_schema.clone())
                .handler(move |args: Value, ctx| {
                    let (url, remote, qualified, ledger) =
                        (url.clone(), remote.clone(), qualified.clone(), ledger.clone());
                    async move {
                        if let Some(why) = tools::blocked_by_security(&ledger, tools::Risk::Egress) {
                            return Ok(json!({"error": why}));
                        }
                        let fut = super::mcp::call(&url, &remote, args.clone());
                        Ok(tools::guard(
                            &ledger,
                            &ctx.session_id,
                            &qualified,
                            &args,
                            std::time::Duration::from_secs(60),
                            Some(ctx.cancellation()),
                            fut,
                        )
                        .await)
                    }
                });
            by_ns.entry(super::mcp::NAMESPACE.to_string()).or_default().push(tool);
        }
    }
    for (ns, list) in by_ns {
        h.register_tools(&ns, list)
            .await
            .map_err(|e| CoworkerError::Setup(format!("Could not register tools: {e}")))?;
    }
    Ok(())
}
