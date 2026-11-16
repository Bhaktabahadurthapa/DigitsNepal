//! Coworker governance store: folder grants, authorized REA targets, the
//! session index and a tamper-evident audit log.
//!
//! Grants and targets are created only by explicit user actions in the UI
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

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// A website origin, e.g. `https://digitsnepal.com`.
    Origin,
    /// A local application or project path.
    Path,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Target {
    pub id: i64,
    pub kind: TargetKind,
    pub value: String,
    /// The user's statement, e.g. "I own this website".
    pub attestation: String,
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
             CREATE TABLE IF NOT EXISTS targets(
               id INTEGER PRIMARY KEY, kind TEXT NOT NULL, value TEXT NOT NULL, attestation TEXT NOT NULL,
               created_at INTEGER NOT NULL, revoked_at INTEGER);
             CREATE TABLE IF NOT EXISTS sessions(
               id TEXT PRIMARY KEY, agent TEXT NOT NULL, specialist TEXT NOT NULL, title TEXT NOT NULL,
               created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
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

    // ---- Authorized REA targets ---------------------------------------------

    pub fn add_target(&self, kind: TargetKind, value: &str, attestation: &str) -> Result<Target, LedgerError> {
        let attestation = attestation.trim();
        if attestation.len() < 8 {
            return Err(LedgerError::Invalid(
                "Confirm that you own or are authorized to inspect this target.".into(),
            ));
        }
        let value = match kind {
            TargetKind::Origin => normalize_origin(value)
                .ok_or_else(|| LedgerError::Invalid("Enter a website address starting with https://".into()))?,
            TargetKind::Path => std::fs::canonicalize(value)
                .map_err(|_| LedgerError::Invalid("That path does not exist.".into()))?
                .to_string_lossy()
                .to_string(),
        };
        let kind_s = match kind {
            TargetKind::Origin => "origin",
            TargetKind::Path => "path",
        };
        let ts = now();
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO targets(kind, value, attestation, created_at) VALUES(?1, ?2, ?3, ?4)",
            params![kind_s, value, attestation, ts],
        )?;
        Ok(Target {
            id: c.last_insert_rowid(),
            kind,
            value,
            attestation: attestation.into(),
            created_at: ts,
        })
    }

    pub fn revoke_target(&self, id: i64) -> Result<(), LedgerError> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE targets SET revoked_at=?1 WHERE id=?2", params![now(), id])?;
        Ok(())
    }

    pub fn targets(&self) -> Result<Vec<Target>, LedgerError> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(
            "SELECT id, kind, value, attestation, created_at FROM targets WHERE revoked_at IS NULL ORDER BY id",
        )?;
        let rows = st.query_map([], |r| {
            let kind: String = r.get(1)?;
            Ok(Target {
                id: r.get(0)?,
                kind: if kind == "origin" {
                    TargetKind::Origin
                } else {
                    TargetKind::Path
                },
                value: r.get(2)?,
                attestation: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
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

/// `https://Example.com/path?q` → `https://example.com`. Only https (or
/// loopback http) origins are accepted.
pub fn normalize_origin(input: &str) -> Option<String> {
    let s = input.trim();
    let (scheme, rest) = s.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let host_port = rest.split(['/', '?', '#']).next()?.to_ascii_lowercase();
    if host_port.is_empty() || host_port.contains('@') {
        return None;
    }
    let host = host_port.split(':').next().unwrap_or_default();
    let loopback = host == "localhost" || host.starts_with("127.") || host == "[::1]";
    match scheme.as_str() {
        "https" => Some(format!("https://{host_port}")),
        "http" if loopback => Some(format!("http://{host_port}")),
        _ => None,
    }
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

    #[test]
    fn targets_require_attestation_and_valid_origin() {
        let l = Ledger::in_memory().unwrap();
        assert!(l
            .add_target(TargetKind::Origin, "https://digitsnepal.com/app", "ok")
            .is_err());
        let t = l
            .add_target(
                TargetKind::Origin,
                "https://DigitsNepal.com/app?x=1",
                "I own this website",
            )
            .unwrap();
        assert_eq!(t.value, "https://digitsnepal.com");
        assert!(l
            .add_target(TargetKind::Origin, "http://example.com", "I own this website")
            .is_err());
        assert!(l
            .add_target(TargetKind::Origin, "ftp://x", "I own this website")
            .is_err());
    }

    #[test]
    fn origin_normalisation() {
        assert_eq!(
            normalize_origin("https://a.com:8443/x").as_deref(),
            Some("https://a.com:8443")
        );
        assert_eq!(
            normalize_origin("http://127.0.0.1:3000/").as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert_eq!(normalize_origin("https://user@a.com"), None);
        assert_eq!(normalize_origin("a.com"), None);
    }
}
