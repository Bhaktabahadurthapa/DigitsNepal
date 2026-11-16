# Agent data model

Two SQLite stores, both local:

- **Engine store** (OpenWorker Harness, `…/coworker/engine`): sessions, history, attempts, submissions, events, teams, memories. Owned by the engine; DigitsNepal reads it only through the SDK.
- **DigitsNepal ledger** (`…/coworker/coworker.db`, WAL): everything governance- and product-related below.

Secrets live only in the macOS Keychain.

## Existing tables (kept)

| Table | Columns |
|---|---|
| `grants` | id, path (canonical), created_at, revoked_at |
| `sessions` | id, agent, specialist, title, created_at, updated_at |
| `messages` | id, session, role, text, ts |
| `kv` | key, value (e.g. relay ports) |
| `audit_log` | seq, ts, session, actor, kind, tool, args_digest, decision, detail, prev_hash, hash |

## New tables

| Table | Columns | Purpose |
|---|---|---|
| `agent_definitions` | id (content digest), name, version, instructions_digest, tools_json, model_connection, model, limits_json, created_at | Versioned, immutable definitions |
| `pending_interactions` | id, session, kind (approval/question), tool, args_preview, args_digest, risk, reason, created_at, resolved_at, decision | Survive restarts; drive the Approval Center |
| `rules` | id, scope (session/folder/host), tool_or_class, target, created_by ('user'), created_at, expires_at, revoked_at | Revocable standing permissions |
| `usage` | session, turn, provider, model, input_tokens, output_tokens, cached_tokens, est_cost_usd, ts | Token and cost tracking |
| `automations` | id, title, instructions, agent_id, schedule (cron/once), folder_scope, enabled, created_at | Scheduled workflows |
| `automation_runs` | id, automation_id, started_at, finished_at, status (ok/failed/waiting/uncertain), session | Run history; "uncertain" never auto-retried |
| `integrations` | id, kind (mcp/github), name, endpoint, enabled, created_at | User-added connections (tokens in Keychain) |
| `tasks` | id, parent_session, agent_id, title, status, depth, budget_json | Orchestrator delegation (Phase 7) |

## Typed records (Rust)

```rust
struct ToolSpec {
    name: &'static str,            // "files.read"
    description: &'static str,
    input_schema: Value,           // strict, additionalProperties: false
    output_schema: Value,
    risk: Risk,                    // Read | WriteLocal | Egress | Exec | External
    timeout: Duration,
    cancellable: bool,
}
struct AgentDefinition { id, name, version, instructions, tools: Vec<String>, model: ModelRef, limits: TurnLimits }
enum Decision { Allow { reason }, Ask { reason, standing_ok, hard_floor }, Deny { reason } }
```

## Rules

- Agent definitions referenced by any session are never modified; a change creates a new version.
- Audit rows are append-only and hash-chained; other tables may update status fields only.
- Tool arguments are stored as digests (plus a short preview for pending cards, deleted when resolved).
- Migrations are additive (`CREATE TABLE IF NOT EXISTS`, new columns with defaults).
