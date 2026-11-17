//! Sahayogi governance store: folder grants, the session index, messages
//! and a tamper-evident audit log.
//!
//! Grants are created only by explicit user actions in the UI
//! (Tauri commands); no agent tool can write to this store. Every audit entry
//! carries the SHA-256 of the previous entry, so any edit or deletion breaks
//! the chain and is reported by [`Ledger::verify_chain`].

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Grant {
    pub id: i64,
    /// Canonical absolute folder path.
    pub path: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AuditEntry {
    pub seq: i64,
    pub ts: i64,
    pub session: String,
    /// `user`, `policy`, `agent` or `system`.
    pub actor: String,
    /// e.g. `tool_call`, `approval`, `grant`, `message`.
    pub kind: String,
    pub tool: String,
    /// SHA-256 of the arguments: the log proves what was approved without
    /// storing document contents.
    pub args_digest: String,
    pub decision: String,
    pub detail: String,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PendingRow {
    pub id: String,
    pub session: String,
    pub kind: String,
    pub tool: String,
    pub preview: Value,
    pub reason: String,
    pub standing_ok: bool,
    pub hard_floor: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SessionRow {
    pub id: String,
    pub agent: String,
    pub specialist: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("storage error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Invalid(String),
}

pub struct Ledger {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TaskRow {
    pub id: String,
    pub session: String,
    pub specialist: String,
    pub request: String,
    /// Offline understanding (`intent::IntentHints`).
    pub intent: Value,
    /// The agent's validated plan, with step statuses.
    pub plan: Value,
    pub evidence: Value,
    pub result: Value,
    pub state: String,
    pub state_reason: String,
    pub paused: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum TaskField {
    Intent,
    Plan,
    Evidence,
    Result,
}

/// Global safety switches, set only by the user in the Security Center.
#[derive(Debug, Clone, Copy, Default, Serialize, serde::Deserialize, PartialEq)]
pub struct Security {
    /// Every tool call is refused.
    pub paused: bool,
    /// Web, GitHub and connected-app tools are refused.
    pub network_off: bool,
    /// `code.run` may be used (still asks each time). Off by default.
    pub code_enabled: bool,
    /// Read-only web research (search, public pages, public GitHub) runs
    /// without asking. Off by default; never covers writes or sending.
    #[serde(default)]
    pub auto_web: bool,
    /// Creating, saving and moving files inside shared folders runs without
    /// asking (existing files are still backed up; nothing is deleted).
    #[serde(default)]
    pub auto_files: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StandingRow {
    pub session: String,
    pub tool: String,
    pub created_at: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutomationRow {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub steps: Value,
    pub created_at: i64,
    pub last_run_at: Option<i64>,
    pub runs: i64,
}

/// A remote MCP server the user added, with the tools it reported and the
/// ones the user enabled for Sahayogi.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
pub struct McpServerRow {
    pub name: String,
    pub url: String,
    pub tools: Vec<McpToolInfo>,
    pub enabled: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
    pub destructive: bool,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Stable digest of tool arguments (object keys are sorted by serde_json's map).
pub fn digest_value(v: &Value) -> String {
    sha256_hex(v.to_string().as_bytes())
}

fn entry_hash(e: &AuditEntry) -> String {
    let material = serde_json::json!([
        e.seq,
        e.ts,
        e.session,
        e.actor,
        e.kind,
        e.tool,
        e.args_digest,
        e.decision,
        e.detail,
        e.prev_hash
    ]);
    sha256_hex(material.to_string().as_bytes())
}

impl Ledger {
    pub fn open(path: &Path) -> Result<Self, LedgerError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| LedgerError::Invalid(e.to_string()))?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, LedgerError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, LedgerError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS grants(
               id INTEGER PRIMARY KEY, path TEXT NOT NULL, created_at INTEGER NOT NULL, revoked_at INTEGER);
             CREATE TABLE IF NOT EXISTS sessions(
               id TEXT PRIMARY KEY, agent TEXT NOT NULL, specialist TEXT NOT NULL, title TEXT NOT NULL,
               created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS agent_definitions(
               id TEXT PRIMARY KEY, key TEXT NOT NULL, version INTEGER NOT NULL, definition TEXT NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS pending_interactions(
               id TEXT PRIMARY KEY, session TEXT NOT NULL, kind TEXT NOT NULL, tool TEXT NOT NULL, preview TEXT NOT NULL,
               reason TEXT NOT NULL, standing_ok INTEGER NOT NULL, hard_floor INTEGER NOT NULL,
               created_at INTEGER NOT NULL, resolved_at INTEGER, decision TEXT);
             CREATE TABLE IF NOT EXISTS usage(
               id INTEGER PRIMARY KEY, session TEXT NOT NULL, model TEXT NOT NULL, input_tokens INTEGER NOT NULL,
               output_tokens INTEGER NOT NULL, ts INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS automations(
               id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, description TEXT NOT NULL, steps TEXT NOT NULL,
               created_at INTEGER NOT NULL, last_run_at INTEGER, runs INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS tasks(
               id TEXT PRIMARY KEY, session TEXT NOT NULL, specialist TEXT NOT NULL, request TEXT NOT NULL,
               intent TEXT NOT NULL, plan TEXT NOT NULL, evidence TEXT NOT NULL, result TEXT NOT NULL,
               state TEXT NOT NULL, state_reason TEXT NOT NULL, paused INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS tasks_session ON tasks(session);
             CREATE TABLE IF NOT EXISTS task_events(
               id INTEGER PRIMARY KEY, task TEXT NOT NULL, ts INTEGER NOT NULL, kind TEXT NOT NULL,
               from_state TEXT NOT NULL, to_state TEXT NOT NULL, detail TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS task_events_task ON task_events(task, id);
             CREATE TABLE IF NOT EXISTS tool_results(
               session TEXT NOT NULL, tool TEXT NOT NULL, digest TEXT NOT NULL, result TEXT NOT NULL, ts INTEGER NOT NULL,
               PRIMARY KEY(session, tool, digest));
             CREATE TABLE IF NOT EXISTS standing_approvals(
               session TEXT NOT NULL, tool TEXT NOT NULL, created_at INTEGER NOT NULL, PRIMARY KEY(session, tool));
             CREATE TABLE IF NOT EXISTS mcp_servers(
               name TEXT PRIMARY KEY, url TEXT NOT NULL, tools TEXT NOT NULL, enabled TEXT NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS messages(
               id INTEGER PRIMARY KEY, session TEXT NOT NULL, role TEXT NOT NULL, text TEXT NOT NULL, ts INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS messages_session ON messages(session, id);
             CREATE TABLE IF NOT EXISTS audit_log(
               seq INTEGER PRIMARY KEY, ts INTEGER NOT NULL, session TEXT NOT NULL, actor TEXT NOT NULL,
               kind TEXT NOT NULL, tool TEXT NOT NULL, args_digest TEXT NOT NULL, decision TEXT NOT NULL,
               detail TEXT NOT NULL, prev_hash TEXT NOT NULL, hash TEXT NOT NULL);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    // ---- Automations ----------------------------------------------------------

    /// Save a new automation; names are unique and an automation never changes.
    pub fn add_automation(&self, name: &str, description: &str, steps: &Value) -> Result<i64, LedgerError> {
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO automations(name, description, steps, created_at) VALUES(?1,?2,?3,?4)",
            params![name, description, steps.to_string(), now()],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn automations(&self) -> Result<Vec<AutomationRow>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(
            "SELECT id, name, description, steps, created_at, last_run_at, runs FROM automations ORDER BY name",
        )?;
        let rows = st.query_map([], |r| {
            Ok(AutomationRow {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                steps: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or(Value::Null),
                created_at: r.get(4)?,
                last_run_at: r.get(5)?,
                runs: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn automation(&self, name: &str) -> Option<AutomationRow> {
        self.automations().ok()?.into_iter().find(|a| a.name == name)
    }

    pub fn mark_automation_run(&self, name: &str) {
        let _ = self.conn.lock().unwrap().execute(
            "UPDATE automations SET runs = runs + 1, last_run_at = ?1 WHERE name = ?2",
            params![now(), name],
        );
    }

    /// Only the user removes automations (from the UI).
    pub fn delete_automation(&self, id: i64) -> Result<(), LedgerError> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM automations WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ---- Tasks ------------------------------------------------------------------

    pub fn insert_task(&self, t: &TaskRow) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id, session, specialist, request, intent, plan, evidence, result, state, state_reason, paused, created_at, updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                t.id, t.session, t.specialist, t.request, t.intent.to_string(), t.plan.to_string(),
                t.evidence.to_string(), t.result.to_string(), t.state, t.state_reason, t.paused as i64,
                t.created_at, t.updated_at
            ],
        )?;
        Ok(())
    }

    fn task_rows(&self, filter: &str, arg: Option<&str>) -> Vec<TaskRow> {
        let c = self.conn.lock().unwrap();
        let sql = format!(
            "SELECT id, session, specialist, request, intent, plan, evidence, result, state, state_reason, paused, created_at, updated_at
             FROM tasks {filter} ORDER BY updated_at DESC LIMIT 200"
        );
        let Ok(mut st) = c.prepare(&sql) else { return Vec::new() };
        let map = |r: &rusqlite::Row| {
            let j = |i: usize| -> rusqlite::Result<Value> {
                Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or(Value::Null))
            };
            Ok(TaskRow {
                id: r.get(0)?,
                session: r.get(1)?,
                specialist: r.get(2)?,
                request: r.get(3)?,
                intent: j(4)?,
                plan: j(5)?,
                evidence: j(6)?,
                result: j(7)?,
                state: r.get(8)?,
                state_reason: r.get(9)?,
                paused: r.get::<_, i64>(10)? != 0,
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
            })
        };
        let rows = match arg {
            Some(a) => st.query_map(params![a], map),
            None => st.query_map([], map),
        };
        rows.map(|r| r.filter_map(Result::ok).collect()).unwrap_or_default()
    }

    pub fn tasks(&self) -> Vec<TaskRow> {
        self.task_rows("", None)
    }

    pub fn task(&self, id: &str) -> Option<TaskRow> {
        self.task_rows("WHERE id = ?1", Some(id)).into_iter().next()
    }

    pub fn task_for_session(&self, session: &str) -> Option<TaskRow> {
        self.task_rows("WHERE session = ?1", Some(session)).into_iter().next()
    }

    /// Update one JSON or text field of a task (field names are fixed here).
    pub fn set_task_field(&self, id: &str, field: TaskField, value: &Value) -> Result<(), LedgerError> {
        let col = match field {
            TaskField::Plan => "plan",
            TaskField::Evidence => "evidence",
            TaskField::Result => "result",
            TaskField::Intent => "intent",
        };
        self.conn.lock().unwrap().execute(
            &format!("UPDATE tasks SET {col} = ?1, updated_at = ?2 WHERE id = ?3"),
            params![value.to_string(), now(), id],
        )?;
        Ok(())
    }

    pub fn set_task_paused(&self, id: &str, paused: bool) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "UPDATE tasks SET paused = ?1, updated_at = ?2 WHERE id = ?3",
            params![paused as i64, now(), id],
        )?;
        Ok(())
    }

    /// Change state only if it is still `from` (compare-and-set), recording
    /// the transition. Returns false if the state had changed meanwhile.
    pub fn set_task_state(&self, id: &str, from: &str, to: &str, reason: &str) -> Result<bool, LedgerError> {
        let c = self.conn.lock().unwrap();
        let n = c.execute(
            "UPDATE tasks SET state = ?1, state_reason = ?2, updated_at = ?3 WHERE id = ?4 AND state = ?5",
            params![to, reason, now(), id, from],
        )?;
        if n == 1 {
            c.execute(
                "INSERT INTO task_events(task, ts, kind, from_state, to_state, detail) VALUES(?1,?2,'state',?3,?4,?5)",
                params![id, now(), from, to, reason],
            )?;
        }
        Ok(n == 1)
    }

    pub fn add_task_event(&self, id: &str, kind: &str, detail: &Value) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO task_events(task, ts, kind, from_state, to_state, detail) VALUES(?1,?2,?3,'','',?4)",
            params![id, now(), kind, detail.to_string()],
        )?;
        Ok(())
    }

    pub fn task_events(&self, id: &str) -> Vec<Value> {
        let c = self.conn.lock().unwrap();
        let Ok(mut st) = c.prepare(
            "SELECT ts, kind, from_state, to_state, detail FROM task_events WHERE task = ?1 ORDER BY id LIMIT 500",
        ) else {
            return Vec::new();
        };
        st.query_map(params![id], |r| {
            let detail: String = r.get(4)?;
            Ok(serde_json::json!({
                "ts": r.get::<_, i64>(0)?,
                "kind": r.get::<_, String>(1)?,
                "from": r.get::<_, String>(2)?,
                "to": r.get::<_, String>(3)?,
                "detail": serde_json::from_str::<Value>(&detail).unwrap_or(Value::String(detail)),
            }))
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
    }

    /// Did this session successfully run `tool` (per the audit log)?
    pub fn session_ran_ok(&self, session: &str, tool: &str) -> bool {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM audit_log WHERE session = ?1 AND kind = 'tool_run' AND tool = ?2 AND decision = 'ok' LIMIT 1",
                params![session, tool],
                |_| Ok(()),
            )
            .is_ok()
    }

    /// Count approvals the user answered in a session (human interventions).
    pub fn session_interventions(&self, session: &str) -> i64 {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM audit_log WHERE session = ?1 AND actor = 'user' AND kind IN ('approval','answer')",
                params![session],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }

    // ---- Idempotency: results of side-effecting tool calls --------------------

    pub fn prior_result(&self, session: &str, tool: &str, digest: &str) -> Option<Value> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT result FROM tool_results WHERE session = ?1 AND tool = ?2 AND digest = ?3",
                params![session, tool, digest],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn save_result(&self, session: &str, tool: &str, digest: &str, result: &Value) {
        let _ = self.conn.lock().unwrap().execute(
            "INSERT OR IGNORE INTO tool_results(session, tool, digest, result, ts) VALUES(?1,?2,?3,?4,?5)",
            params![session, tool, digest, result.to_string(), now()],
        );
    }

    // ---- Security settings and standing approvals ---------------------------

    pub fn security(&self) -> Security {
        let flag = |k: &str| self.kv_get(k).is_some_and(|v| v == "1");
        Security {
            paused: flag("security.paused"),
            network_off: flag("security.network_off"),
            code_enabled: flag("security.code_enabled"),
            auto_web: flag("security.auto_web"),
            auto_files: flag("security.auto_files"),
        }
    }

    pub fn set_security(&self, s: &Security) -> Result<(), LedgerError> {
        let b = |v: bool| if v { "1" } else { "0" };
        self.kv_set("security.paused", b(s.paused))?;
        self.kv_set("security.network_off", b(s.network_off))?;
        self.kv_set("security.code_enabled", b(s.code_enabled))?;
        self.kv_set("security.auto_web", b(s.auto_web))?;
        self.kv_set("security.auto_files", b(s.auto_files))?;
        Ok(())
    }

    pub fn add_standing(&self, session: &str, tool: &str) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT OR IGNORE INTO standing_approvals(session, tool, created_at) VALUES(?1,?2,?3)",
            params![session, tool, now()],
        )?;
        Ok(())
    }

    pub fn has_standing(&self, session: &str, tool: &str) -> bool {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM standing_approvals WHERE session = ?1 AND tool = ?2",
                params![session, tool],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn standing(&self) -> Vec<StandingRow> {
        let c = self.conn.lock().unwrap();
        let Ok(mut st) = c.prepare(
            "SELECT a.session, a.tool, a.created_at, COALESCE(s.title, '') FROM standing_approvals a
             LEFT JOIN sessions s ON s.id = a.session ORDER BY a.created_at DESC",
        ) else {
            return Vec::new();
        };
        st.query_map([], |r| {
            Ok(StandingRow {
                session: r.get(0)?,
                tool: r.get(1)?,
                created_at: r.get(2)?,
                title: r.get(3)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
    }

    /// Revoke one standing approval, or all of them when `tool` is None.
    pub fn revoke_standing(&self, session: Option<&str>, tool: Option<&str>) -> Result<usize, LedgerError> {
        let c = self.conn.lock().unwrap();
        Ok(match (session, tool) {
            (Some(s), Some(t)) => c.execute(
                "DELETE FROM standing_approvals WHERE session = ?1 AND tool = ?2",
                params![s, t],
            )?,
            (Some(s), None) => c.execute("DELETE FROM standing_approvals WHERE session = ?1", params![s])?,
            _ => c.execute("DELETE FROM standing_approvals", [])?,
        })
    }

    // ---- MCP servers ----------------------------------------------------------

    pub fn save_mcp_server(&self, row: &McpServerRow) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO mcp_servers(name, url, tools, enabled, created_at) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(name) DO UPDATE SET tools=excluded.tools, enabled=excluded.enabled",
            params![
                row.name,
                row.url,
                serde_json::to_string(&row.tools).unwrap_or_default(),
                serde_json::to_string(&row.enabled).unwrap_or_default(),
                row.created_at
            ],
        )?;
        Ok(())
    }

    pub fn mcp_servers(&self) -> Vec<McpServerRow> {
        let c = self.conn.lock().unwrap();
        let Ok(mut st) = c.prepare("SELECT name, url, tools, enabled, created_at FROM mcp_servers ORDER BY name")
        else {
            return Vec::new();
        };
        st.query_map([], |r| {
            Ok(McpServerRow {
                name: r.get(0)?,
                url: r.get(1)?,
                tools: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
                enabled: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                created_at: r.get(4)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
    }

    pub fn delete_mcp_server(&self, name: &str) -> Result<(), LedgerError> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM mcp_servers WHERE name = ?1", params![name])?;
        Ok(())
    }

    // ---- Folder grants ------------------------------------------------------

    /// Grant the coworker access to a folder. The path is canonicalised so a
    /// symlink cannot later widen the grant.
    pub fn add_grant(&self, folder: &Path) -> Result<Grant, LedgerError> {
        let canon =
            std::fs::canonicalize(folder).map_err(|_| LedgerError::Invalid("That folder does not exist.".into()))?;
        if !canon.is_dir() {
            return Err(LedgerError::Invalid("Choose a folder, not a file.".into()));
        }
        if canon.parent().is_none() {
            return Err(LedgerError::Invalid("Granting the whole disk is not allowed.".into()));
        }
        let path = canon.to_string_lossy().to_string();
        let c = self.conn.lock().unwrap();
        if let Some(g) = c
            .query_row(
                "SELECT id, path, created_at FROM grants WHERE path=?1 AND revoked_at IS NULL",
                [&path],
                |r| {
                    Ok(Grant {
                        id: r.get(0)?,
                        path: r.get(1)?,
                        created_at: r.get(2)?,
                    })
                },
            )
            .optional()?
        {
            return Ok(g);
        }
        let ts = now();
        c.execute("INSERT INTO grants(path, created_at) VALUES(?1, ?2)", params![path, ts])?;
        Ok(Grant {
            id: c.last_insert_rowid(),
            path,
            created_at: ts,
        })
    }

    pub fn revoke_grant(&self, id: i64) -> Result<(), LedgerError> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE grants SET revoked_at=?1 WHERE id=?2", params![now(), id])?;
        Ok(())
    }

    pub fn grants(&self) -> Result<Vec<Grant>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare("SELECT id, path, created_at FROM grants WHERE revoked_at IS NULL ORDER BY id")?;
        let rows = st.query_map([], |r| {
            Ok(Grant {
                id: r.get(0)?,
                path: r.get(1)?,
                created_at: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn granted_roots(&self) -> Vec<PathBuf> {
        self.grants()
            .unwrap_or_default()
            .into_iter()
            .map(|g| PathBuf::from(g.path))
            .collect()
    }

    // ---- Sessions -----------------------------------------------------------

    pub fn upsert_session(&self, row: &SessionRow) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO sessions(id, agent, specialist, title, created_at, updated_at) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, updated_at=excluded.updated_at",
            params![
                row.id,
                row.agent,
                row.specialist,
                row.title,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn sessions(&self) -> Result<Vec<SessionRow>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(
            "SELECT id, agent, specialist, title, created_at, updated_at FROM sessions ORDER BY updated_at DESC LIMIT 200",
        )?;
        let rows = st.query_map([], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                agent: r.get(1)?,
                specialist: r.get(2)?,
                title: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- Agent definitions (immutable) ------------------------------------

    /// Store a definition once. An existing ID is never overwritten, so a
    /// session always resumes with the exact definition it started with.
    pub fn save_definition(&self, id: &str, key: &str, version: u32, definition: &Value) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT OR IGNORE INTO agent_definitions(id, key, version, definition, created_at) VALUES(?1,?2,?3,?4,?5)",
            params![id, key, version, definition.to_string(), now()],
        )?;
        Ok(())
    }

    pub fn definition(&self, id: &str) -> Option<Value> {
        let c = self.conn.lock().unwrap();
        c.query_row("SELECT definition FROM agent_definitions WHERE id=?1", [id], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
    }

    // ---- Pending interactions (survive restarts) -------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn add_pending(
        &self,
        id: &str,
        session: &str,
        kind: &str,
        tool: &str,
        preview: &Value,
        reason: &str,
        standing_ok: bool,
        hard_floor: bool,
    ) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO pending_interactions(id, session, kind, tool, preview, reason, standing_ok, hard_floor, created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET resolved_at=NULL, decision=NULL",
            params![id, session, kind, tool, preview.to_string(), reason, standing_ok, hard_floor, now()],
        )?;
        Ok(())
    }

    pub fn resolve_pending(&self, id: &str, decision: &str) -> Result<(), LedgerError> {
        // The preview is cleared once answered; only the decision remains.
        self.conn.lock().unwrap().execute(
            "UPDATE pending_interactions SET resolved_at=?1, decision=?2, preview='null' WHERE id=?3 AND resolved_at IS NULL",
            params![now(), decision, id],
        )?;
        Ok(())
    }

    pub fn unresolved_pending(&self) -> Result<Vec<PendingRow>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(
            "SELECT id, session, kind, tool, preview, reason, standing_ok, hard_floor, created_at
             FROM pending_interactions WHERE resolved_at IS NULL ORDER BY created_at",
        )?;
        let rows = st.query_map([], |r| {
            Ok(PendingRow {
                id: r.get(0)?,
                session: r.get(1)?,
                kind: r.get(2)?,
                tool: r.get(3)?,
                preview: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or(Value::Null),
                reason: r.get(5)?,
                standing_ok: r.get(6)?,
                hard_floor: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- Token usage -----------------------------------------------------

    pub fn add_usage(&self, session: &str, model: &str, input: u64, output: u64) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO usage(session, model, input_tokens, output_tokens, ts) VALUES(?1,?2,?3,?4,?5)",
            params![session, model, input as i64, output as i64, now()],
        )?;
        Ok(())
    }

    /// (input, output) tokens for one session, or all sessions when `None`.
    pub fn usage_totals(&self, session: Option<&str>) -> (u64, u64) {
        let c = self.conn.lock().unwrap();
        let q = "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) FROM usage WHERE (?1 IS NULL OR session=?1)";
        c.query_row(q, [session], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
        })
        .unwrap_or((0, 0))
    }

    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT value FROM kv WHERE key=?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO kv(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn add_message(&self, session: &str, role: &str, text: &str) -> Result<(), LedgerError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO messages(session, role, text, ts) VALUES(?1, ?2, ?3, ?4)",
            params![session, role, text, now()],
        )?;
        Ok(())
    }

    pub fn messages(&self, session: &str) -> Result<Vec<(String, String, i64)>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare("SELECT role, text, ts FROM messages WHERE session=?1 ORDER BY id LIMIT 1000")?;
        let rows = st.query_map([session], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- Audit log ----------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        session: &str,
        actor: &str,
        kind: &str,
        tool: &str,
        args: &Value,
        decision: &str,
        detail: &str,
    ) -> Result<AuditEntry, LedgerError> {
        let c = self.conn.lock().unwrap();
        let (seq, prev_hash) = c
            .query_row("SELECT seq, hash FROM audit_log ORDER BY seq DESC LIMIT 1", [], |r| {
                Ok((r.get::<_, i64>(0)? + 1, r.get::<_, String>(1)?))
            })
            .optional()?
            .unwrap_or((1, GENESIS.to_string()));
        let mut e = AuditEntry {
            seq,
            ts: now(),
            session: session.into(),
            actor: actor.into(),
            kind: kind.into(),
            tool: tool.into(),
            args_digest: if args.is_null() {
                String::new()
            } else {
                digest_value(args)
            },
            decision: decision.into(),
            detail: detail.chars().take(500).collect(),
            prev_hash,
            hash: String::new(),
        };
        e.hash = entry_hash(&e);
        c.execute(
            "INSERT INTO audit_log(seq, ts, session, actor, kind, tool, args_digest, decision, detail, prev_hash, hash)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                e.seq,
                e.ts,
                e.session,
                e.actor,
                e.kind,
                e.tool,
                e.args_digest,
                e.decision,
                e.detail,
                e.prev_hash,
                e.hash
            ],
        )?;
        Ok(e)
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<AuditEntry>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(
            "SELECT seq, ts, session, actor, kind, tool, args_digest, decision, detail, prev_hash, hash
             FROM audit_log ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit as i64], row_to_entry)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Re-hash the whole log. Returns the first broken sequence number, if any.
    pub fn verify_chain(&self) -> Result<Option<i64>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(
            "SELECT seq, ts, session, actor, kind, tool, args_digest, decision, detail, prev_hash, hash
             FROM audit_log ORDER BY seq ASC",
        )?;
        let mut prev = GENESIS.to_string();
        for (expected_seq, e) in (1..).zip(st.query_map([], row_to_entry)?) {
            let e = e?;
            if e.seq != expected_seq || e.prev_hash != prev || entry_hash(&e) != e.hash {
                return Ok(Some(e.seq));
            }
            prev = e.hash;
        }
        Ok(None)
    }

    #[cfg(test)]
    pub(crate) fn tamper(&self, sql: &str) {
        self.conn.lock().unwrap().execute_batch(sql).unwrap();
    }
}

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<AuditEntry> {
    Ok(AuditEntry {
        seq: r.get(0)?,
        ts: r.get(1)?,
        session: r.get(2)?,
        actor: r.get(3)?,
        kind: r.get(4)?,
        tool: r.get(5)?,
        args_digest: r.get(6)?,
        decision: r.get(7)?,
        detail: r.get(8)?,
        prev_hash: r.get(9)?,
        hash: r.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grants_are_canonical_and_revocable() {
        let l = Ledger::in_memory().unwrap();
        let d = tempfile::tempdir().unwrap();
        let g = l.add_grant(d.path()).unwrap();
        assert_eq!(
            l.add_grant(d.path()).unwrap().id,
            g.id,
            "same folder is not granted twice"
        );
        assert_eq!(l.grants().unwrap().len(), 1);
        l.revoke_grant(g.id).unwrap();
        assert!(l.grants().unwrap().is_empty());
        assert!(l.add_grant(&d.path().join("missing")).is_err());
        assert!(l.add_grant(Path::new("/")).is_err(), "whole disk cannot be granted");
    }

    #[test]
    fn audit_chain_detects_edits_and_deletions() {
        let l = Ledger::in_memory().unwrap();
        for i in 0..5 {
            l.record(
                "s1",
                "policy",
                "tool_call",
                "dn.read_file",
                &json!({"path": i}),
                "allow",
                "inside grant",
            )
            .unwrap();
        }
        assert_eq!(l.verify_chain().unwrap(), None);
        l.tamper("UPDATE audit_log SET decision='deny' WHERE seq=3");
        assert_eq!(l.verify_chain().unwrap(), Some(3));

        let l = Ledger::in_memory().unwrap();
        for i in 0..4 {
            l.record(
                "s1",
                "user",
                "approval",
                "dn.write_file",
                &json!({"i": i}),
                "approve_once",
                "",
            )
            .unwrap();
        }
        l.tamper("DELETE FROM audit_log WHERE seq=2");
        assert_eq!(l.verify_chain().unwrap(), Some(3));
    }

    #[test]
    fn definitions_are_immutable_once_saved() {
        let l = Ledger::in_memory().unwrap();
        l.save_definition("dn-a-v1-x", "a", 1, &json!({"instructions": "one"}))
            .unwrap();
        l.save_definition("dn-a-v1-x", "a", 1, &json!({"instructions": "CHANGED"}))
            .unwrap();
        assert_eq!(l.definition("dn-a-v1-x").unwrap()["instructions"], "one");
        assert!(l.definition("missing").is_none());
    }

    #[test]
    fn pending_interactions_survive_until_resolved() {
        let path = tempfile::tempdir().unwrap().keep().join("c.db");
        {
            let l = Ledger::open(&path).unwrap();
            l.add_pending(
                "i1",
                "s1",
                "approval",
                "files.create",
                &json!({"path": "/x"}),
                "writes",
                true,
                false,
            )
            .unwrap();
            l.add_pending("i2", "s1", "approval", "files.move", &json!({}), "moves", true, false)
                .unwrap();
            l.resolve_pending("i2", "deny").unwrap();
        }
        let l = Ledger::open(&path).unwrap();
        let open = l.unresolved_pending().unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(
            (open[0].id.as_str(), open[0].preview["path"].as_str()),
            ("i1", Some("/x"))
        );
        l.resolve_pending("i1", "once").unwrap();
        assert!(l.unresolved_pending().unwrap().is_empty());
    }

    #[test]
    fn usage_totals_per_session_and_overall() {
        let l = Ledger::in_memory().unwrap();
        l.add_usage("s1", "m", 100, 20).unwrap();
        l.add_usage("s1", "m", 50, 5).unwrap();
        l.add_usage("s2", "m", 10, 1).unwrap();
        assert_eq!(l.usage_totals(Some("s1")), (150, 25));
        assert_eq!(l.usage_totals(None), (160, 26));
    }

    #[test]
    fn audit_stores_digests_not_contents() {
        let l = Ledger::in_memory().unwrap();
        let e = l
            .record(
                "s",
                "agent",
                "tool_call",
                "dn.write_file",
                &json!({"content": "secret salary numbers"}),
                "ask",
                "",
            )
            .unwrap();
        assert_eq!(e.args_digest.len(), 64);
        let all = serde_json::to_string(&l.recent(10).unwrap()).unwrap();
        assert!(!all.contains("salary"));
    }
}
