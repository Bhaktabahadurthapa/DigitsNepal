//! Coworker runtime: the OpenWorker Harness (embedded, in-process) driven by
//! DigitsNepal's policy engine.
//!
//! Every agent runs with `Approvals::Always`, so the engine pauses before each
//! tool call. This service answers each pause with `policy::evaluate`:
//! allowed calls continue automatically, denied calls are refused, and the
//! rest wait for the user's decision in the UI. All outcomes go to the
//! tamper-evident audit log.

use super::files;
use super::language;
use super::ledger::{Ledger, SessionRow};
use super::policy::{self, Decision, ToolHints};
use super::providers::{self, ProviderSpec};
use super::rea::{self, Investigation};
use super::specialists::{self, Specialist};
use openworker_sdk::agents::*;
use openworker_sdk::{Agent, ApprovalDecision, Approvals, BuiltinTools, Harness, McpServer, Sandbox, Tool};
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
    mcp: HashSet<String>,
}

pub struct Config {
    pub state_dir: PathBuf,
    /// Engine working directory for sessions; must not overlap `state_dir`.
    pub workspace_dir: PathBuf,
    pub backup_dir: PathBuf,
    /// Absolute path of the bundled `openworker-harness` runner, if known.
    pub runner: Option<PathBuf>,
}

pub struct Coworker {
    pub ledger: Arc<Ledger>,
    cfg: Config,
    sink: Sink,
    engine: tokio::sync::Mutex<Option<Engine>>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    pending: Mutex<HashMap<String, Pending>>,
    hints: RwLock<HashMap<String, ToolHints>>,
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
    CoworkerError::Engine(e.to_string())
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
        })
    }

    fn emit(&self, session: &str, kind: &str, mut payload: Value) {
        payload["session"] = json!(session);
        payload["type"] = json!(kind);
        (self.sink)(payload);
    }

    // ---- Engine lifecycle ---------------------------------------------------

    async fn ensure_engine(&self) -> Result<tokio::sync::MutexGuard<'_, Option<Engine>>, CoworkerError> {
        let mut guard = self.engine.lock().await;
        if guard.is_none() {
            std::fs::create_dir_all(&self.cfg.state_dir).map_err(|e| CoworkerError::Setup(e.to_string()))?;
            let mut b = Harness::builder()
                .state_dir(self.cfg.state_dir.clone())
                .sandbox(Sandbox::Off)
                .persist_sessions(true);
            if let Some(r) = &self.cfg.runner {
                b = b.executable(r.clone());
            }
            let harness = b
                .start()
                .await
                .map_err(|e| CoworkerError::Setup(format!("Sahayogi could not start: {e}")))?;
            register_local_tools(&harness, self.ledger.clone(), self.cfg.backup_dir.clone()).await?;
            *guard = Some(Engine {
                harness,
                connections: HashMap::new(),
                relays: HashMap::new(),
                agents: HashSet::new(),
                mcp: HashSet::new(),
            });
        }
        Ok(guard)
    }

    pub async fn shutdown(&self) {
        if let Some(e) = self.engine.lock().await.take() {
            let _ = e.harness.shutdown().await;
        }
    }

    /// Register (once) the model connection and the specialist's agent, and
    /// return the agent ID. Agent definitions are immutable in the engine, so
    /// the ID includes a digest of everything that defines it.
    async fn prepare_agent(
        &self,
        spec: &ProviderSpec,
        specialist: Specialist,
        investigation: Option<Investigation>,
    ) -> Result<String, CoworkerError> {
        let mut guard = self.ensure_engine().await?;
        let engine = guard.as_mut().expect("engine started");
        let connection = match engine.connections.get(&spec.connection) {
            Some(c) => c.clone(),
            None => {
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
                name
            }
        };
        let spec = &ProviderSpec {
            connection,
            ..spec.clone()
        };
        let (name, instructions, tools): (&str, &str, Vec<String>) = match specialist {
            Specialist::General => (
                "Sahayogi",
                specialists::GENERAL_INSTRUCTIONS,
                specialists::LOCAL_TOOLS.iter().map(|s| s.to_string()).collect(),
            ),
            Specialist::Rea => {
                let inv = investigation.unwrap_or(Investigation::Website);
                let ns = format!("rea_{}", inv.id());
                let (ready, inst) = rea::readiness();
                let inst = inst.ok_or_else(|| CoworkerError::Setup(ready.problems.join(" ")))?;
                let families = inv.families();
                let catalog = tokio::task::spawn_blocking({
                    let inst = inst.clone();
                    move || rea::discover_tools(&inst, families, std::time::Duration::from_secs(120))
                })
                .await
                .map_err(|e| CoworkerError::Setup(e.to_string()))?
                .map_err(CoworkerError::Setup)?;
                if catalog.is_empty() {
                    return Err(CoworkerError::Setup(
                        "App Insights has no tools for this kind of investigation.".into(),
                    ));
                }
                // Engine MCP identities are immutable: version and scope are part of the name.
                let ns = format!(
                    "{ns}_{}",
                    &super::ledger::sha256_hex(format!("{}{families:?}", inst.version).as_bytes())[..6]
                );
                if engine.mcp.insert(ns.clone()) {
                    let (program, args) = rea::launch_args(&inst, families);
                    engine
                        .harness
                        .start_mcp_server(McpServer::stdio(ns.clone(), program, args))
                        .await
                        .map_err(|e| CoworkerError::Setup(format!("Could not start App Insights: {e}")))?;
                }
                self.hints.write().unwrap().extend(rea::hints_for(&ns, &catalog));
                let mut tools: Vec<String> = catalog.iter().map(|t| format!("{ns}.{}", t.name)).collect();
                tools.push("dn.understand_text".into());
                ("Sahayogi App Insights", specialists::REA_INSTRUCTIONS, tools)
            }
        };
        let digest = super::ledger::sha256_hex(
            format!("{}|{}|{}|{}|{tools:?}", spec.connection, spec.model, name, instructions).as_bytes(),
        );
        let id = format!(
            "dn-{}-{}",
            if specialist == Specialist::General {
                "general"
            } else {
                "rea"
            },
            &digest[..10]
        );
        if !engine.agents.contains(&id) {
            engine
                .harness
                .create_agent(
                    Agent::new(id.clone())
                        .name(name)
                        .instructions(instructions)
                        .model(spec.connection.clone(), spec.model.clone())
                        .tools(tools)
                        .approvals(Approvals::Always)
                        .builtin_tools(BuiltinTools {
                            ask_user: Some(true),
                            read_history: Some(false),
                        })
                        .planning(false)
                        .memory(false)
                        .scheduling(false),
                )
                .await
                .map_err(engine_err)?;
            engine.agents.insert(id.clone());
        }
        Ok(id)
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

    pub async fn new_session(
        &self,
        spec: &ProviderSpec,
        specialist: Specialist,
        investigation: Option<Investigation>,
    ) -> Result<SessionRow, CoworkerError> {
        let agent = self.prepare_agent(spec, specialist, investigation).await?;
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
            specialist: match specialist {
                Specialist::General => "general".into(),
                Specialist::Rea => format!("rea_{}", investigation.unwrap_or(Investigation::Website).id()),
            },
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
        let (specialist, inv) = match row.specialist.as_str() {
            "general" => (Specialist::General, None),
            "rea_js" => (Specialist::Rea, Some(Investigation::JavascriptApp)),
            "rea_desktop" => (Specialist::Rea, Some(Investigation::DesktopApp)),
            _ => (Specialist::Rea, Some(Investigation::Website)),
        };
        let agent = self.prepare_agent(spec, specialist, inv).await?;
        if agent != row.agent {
            return Err(CoworkerError::Setup(
                "The model or specialist changed since this conversation started. Start a new conversation.".into(),
            ));
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
        let session = self.session(session_id, spec).await?;
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
        let me = self.clone();
        let sid = session_id.to_string();
        let input = text.to_string();
        tokio::spawn(async move {
            me.emit(&sid, "status", json!({"state": "running"}));
            match me.drive(&sid, &session, &input).await {
                Ok(()) => {}
                Err(e) => {
                    me.emit(&sid, "error", json!({"message": e.to_string()}));
                    me.ledger
                        .record(&sid, "system", "error", "", &Value::Null, "failed", &e.to_string())
                        .ok();
                }
            }
            me.emit(&sid, "status", json!({"state": "idle"}));
        });
        Ok(())
    }

    async fn drive(&self, sid: &str, session: &Session, input: &str) -> Result<(), CoworkerError> {
        let mut stream = session.run_streamed(input).await.map_err(engine_err)?;
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
                let targets = self.ledger.targets().unwrap_or_default();
                let hints = self.hints.read().unwrap().clone();
                let decision = policy::evaluate(
                    &q,
                    arguments,
                    &policy::Context {
                        roots: &roots,
                        targets: &targets,
                        hints: &hints,
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
                    Decision::Ask {
                        reason,
                        standing_ok,
                        hard_floor,
                    } => {
                        self.ledger
                            .record(sid, "policy", "tool_call", &q, arguments, "ask", reason)
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
                        self.emit(sid, "approval_resolved", json!({"id": i.id, "decision": effective}));
                        match effective {
                            UserDecision::Once => Response::Approve(ApprovalDecision::Once),
                            UserDecision::Session => Response::Approve(ApprovalDecision::Session),
                            UserDecision::Deny => Response::Deny,
                        }
                    }
                };
                session.respond(i, response).await.map_err(engine_err)
            }
            InterruptionKind::Question { question, options } => {
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
                session.respond(i, Response::Answer(answer)).await.map_err(engine_err)
            }
            other => Err(CoworkerError::Engine(format!(
                "The engine paused for a safety check: {other:?}"
            ))),
        }
    }

    pub fn decide(&self, id: &str, decision: UserDecision) -> bool {
        match self.pending.lock().unwrap().remove(id) {
            Some(Pending::Approval { tx, .. }) => tx.send(decision).is_ok(),
            Some(other) => {
                self.pending.lock().unwrap().insert(id.to_string(), other);
                false
            }
            None => false,
        }
    }

    pub fn answer(&self, id: &str, text: &str) -> bool {
        match self.pending.lock().unwrap().remove(id) {
            Some(Pending::Question { tx, .. }) => tx.send(text.to_string()).is_ok(),
            Some(other) => {
                self.pending.lock().unwrap().insert(id.to_string(), other);
                false
            }
            None => false,
        }
    }

    /// Stop the session's turn. Pending approvals are denied, never approved.
    pub async fn cancel(&self, session_id: &str) {
        self.pending.lock().unwrap().retain(|_, p| match p {
            Pending::Approval { session, .. } | Pending::Question { session, .. } => session != session_id,
        });
        let s = self.sessions.lock().unwrap().get(session_id).cloned();
        if let Some(s) = s {
            let _ = s.cancel().await;
        }
        self.ledger
            .record(session_id, "user", "cancel", "", &Value::Null, "cancelled", "")
            .ok();
    }
}

fn preview(v: Option<&Value>) -> Value {
    match v {
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

fn tool_result(r: Result<Value, files::FileError>) -> anyhow::Result<Value> {
    // Errors are returned to the model as data so it can explain them.
    Ok(r.unwrap_or_else(|e| json!({"error": e.to_string()})))
}

fn arg<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// DigitsNepal's own tools (namespace `dn`). Each handler re-checks the
/// folder grant at execution time, independently of the policy decision.
async fn register_local_tools(h: &Harness, ledger: Arc<Ledger>, backups: PathBuf) -> Result<(), CoworkerError> {
    let path_schema = |extra: Value| {
        let mut props = json!({"path": {"type": "string", "description": "Absolute path inside a granted folder"}});
        if let (Some(p), Some(e)) = (props.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        let mut required = vec!["path"];
        if extra.get("content").is_some() {
            required.push("content");
        }
        json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
    };
    let l1 = ledger.clone();
    let l2 = ledger.clone();
    let l3 = ledger.clone();
    let l4 = ledger.clone();
    let l5 = ledger.clone();
    let tools = [
        Tool::new("understand_text")
            .description("Offline Nepali understanding: detect Romanized Nepali / Devanagari / English / mixed and return dictionary glosses. Call first for any Nepali or mixed request.")
            .input_schema(json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}))
            .handler(|args: Value, _| async move { Ok(language::understand_text(arg(&args, "text"))) }),
        Tool::new("granted_folders")
            .description("List the folders the user granted to DigitsNepal (full paths) and the user's home folder. Call this before any file task instead of guessing paths.")
            .input_schema(json!({"type":"object","properties":{},"additionalProperties":false}))
            .handler({
                let l = ledger.clone();
                move |_: Value, _| {
                    let l = l.clone();
                    async move {
                        let roots: Vec<String> = l.granted_roots().iter().map(|p| p.display().to_string()).collect();
                        Ok(json!({
                            "granted_folders": roots,
                            "home": std::env::var("HOME").unwrap_or_default(),
                            "note": if roots.is_empty() { "No folders granted yet. Ask the user to add one under Sahayogi → Folders." } else { "" }
                        }))
                    }
                }
            }),
        Tool::new("list_folder")
            .description("List a folder inside a granted folder (newest first).")
            .input_schema(path_schema(json!({})))
            .handler(move |args: Value, _| {
                let l = l1.clone();
                async move { tool_result(files::list_folder(arg(&args, "path"), &l.granted_roots())) }
            }),
        Tool::new("search_files")
            .description("Find files by name (case-insensitive) under a granted folder; optional extension such as \"pdf\".")
            .input_schema(json!({"type":"object","properties":{
                "folder":{"type":"string","description":"Absolute folder path inside a granted folder"},
                "query":{"type":"string","description":"Part of the file name; empty matches all"},
                "extension":{"type":"string"}},"required":["folder"],"additionalProperties":false}))
            .handler(move |args: Value, _| {
                let l = l2.clone();
                async move {
                    let ext = args.get("extension").and_then(Value::as_str).filter(|s| !s.is_empty());
                    tool_result(files::search_files(arg(&args, "folder"), arg(&args, "query"), ext, &l.granted_roots()))
                }
            }),
        Tool::new("read_file")
            .description("Read a UTF-8 text file inside a granted folder (up to 256 KB).")
            .input_schema(path_schema(json!({})))
            .handler(move |args: Value, _| {
                let l = l3.clone();
                async move { tool_result(files::read_file(arg(&args, "path"), &l.granted_roots())) }
            }),
        Tool::new("read_pdf")
            .description("Extract the text of a PDF inside a granted folder.")
            .input_schema(path_schema(json!({})))
            .handler(move |args: Value, _| {
                let l = l4.clone();
                async move {
                    let path = arg(&args, "path").to_string();
                    let roots = l.granted_roots();
                    tool_result(
                        tokio::task::spawn_blocking(move || files::read_pdf(&path, &roots))
                            .await
                            .unwrap_or_else(|e| Err(files::FileError::Io(e.to_string()))),
                    )
                }
            }),
        Tool::new("write_file")
            .description("Create or replace a text file inside a granted folder. Always asks the user first; an existing file is backed up.")
            .input_schema(path_schema(json!({"content": {"type": "string"}})))
            .handler(move |args: Value, _| {
                let l = l5.clone();
                let b = backups.clone();
                async move { tool_result(files::write_file(arg(&args, "path"), arg(&args, "content"), &l.granted_roots(), &b)) }
            }),
    ];
    h.register_tools("dn", tools)
        .await
        .map_err(|e| CoworkerError::Setup(format!("Could not register tools: {e}")))
}
