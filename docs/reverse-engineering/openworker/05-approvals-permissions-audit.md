# 8 Human approvals · 9 Permission management · 14 Audit logging

## 8. Human approval system

- **User workflow**: an approval card shows the proposed tool and arguments → Allow once / allow for the session / Deny. Unattended runs park the request in the Inbox.
- **Desktop**: when the permission engine returns `needs_user`, the engine emits `PERMISSION_REQUIRED` and awaits an injected async approver (`engine.py` header; `PermissionRequest`, `engine.py:117`; `ApprovalOutcome`, `engine.py:85`). Every pending prompt lives in the durable wait queue (`coworker/inbox.py`), so it survives a dropped socket or restart; only Unattended sessions route to the cross-session Inbox.
- **Reviewer**: `AUTO_APPROVE` mode consults an LLM reviewer (session model or decision models, `decision_reviewer.py`); it can only turn "ask" into "allow". Circuit breaker: `_REVIEWER_TRIP = 5` consecutive denials (`engine.py:70`, `engine.py:2346`) hands control back to the human.
- **Harness**: `Approvals::{Always, Only(set), Reviewer, Auto(model), Never}` (`harness-sdk/src/agents/mod.rs:84`); `Interruption { kind: Approval { tool, qualified_name, arguments, category } }`; `Response::Approve(ApprovalDecision::{Once, Session}) | Deny`; session grants bound to bundle, schema and policy digests (`runtime/src/permissions.rs:26`).
- **Complexity**: High. **Confidence**: High.

## 9. Permission management

- **Desktop modes** (`coworker/permissions.py:551`): `DISCUSS`, `PLAN` (read-only), `INTERACTIVE` (default), `AUTO_APPROVE`, `CUSTOM` (config allowlist), `BYPASS_APPROVALS`, `DANGEROUSLY_BYPASS_APPROVALS` (never offered in the desktop picker).
- **Hard floors** survive bypass: self-protection of OpenWorker's settings (a refusal), writes outside session folders, files that execute later (git hooks, CI configs), running downloaded files, authority that outlives the session. `Decision { allowed, reason, needs_user, human_only, rule, site, network_hosts }` (`permissions.py:603`); `human_only` asks can never be cleared by the reviewer.
- **Earned autonomy**: one-off approval → task-scoped standing rule ("tool → target") → config allowlist; each step explicit and revocable (README "Governed by design").
- **Harness**: `permissions::Mode { Ask, AutoApprove, Bypass }`, `Category`, `hard_check*` functions (`permissions.rs:10-160`).
- **Complexity**: High (1,846-line permissions module). **Confidence**: High.

## 14. Audit logging

- **Desktop**: `AuditStore` (`coworker/audit.py:44`), SQLite table `audit_events`; arguments and results sanitized — secret-looking keys redacted at every nesting level, values truncated to 500 characters (`audit.py:13-30`, `_sanitize_*`). Provenance of each decision (auto, user, denied, reviewer reasoning) recorded (`provenance.py`).
- **Harness**: durable `events` journal; `RunEvent::ApprovalDecision { verdict, origin, reason }`; optional compliance export (`compliance` feature).
- **Gap noted**: neither log is tamper-evident (no hash chain observed). DigitsNepal's ledger already adds a SHA-256 chain.
- **Complexity**: Medium. **Confidence**: High (Desktop), Medium (Harness compliance details).
