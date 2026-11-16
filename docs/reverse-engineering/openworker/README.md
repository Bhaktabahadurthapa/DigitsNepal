# OpenWorker investigation (Phase 1)

Purpose: understand how OpenWorker implements its AI-coworker capabilities so
DigitsNepal can build original equivalents. Nothing here is copied code.

## Sources

| Source | Identity | Method |
|---|---|---|
| OpenWorker repository (public, MIT) | `andrewyng/openworker` @ `ecb4454` (2026-11-13, latest `main`) | Source reading |
| OpenWorker Desktop App (shipped) | `OpenWorker-macos-arm64.dmg`, SHA-256 `3fb2f173…82dc`, app 0.3.1 | REA static analysis, never installed or run |
| Harness SDK docs | `docs/harness/guides/*` in the same repo | Reading |

REA guidance followed: public source is read directly; REA is used where a claim
depends on the shipped artifact (packaging, signing, entitlements, bundled
components). See [shipped-app.md](shipped-app.md).

## Confidence legend

- **Observed (REA)**: read from the shipped app by REA, with an Evidence ID.
- **High**: read directly in source at the cited file and line.
- **Medium**: source plus documentation; some internals not traced end to end.
- **Unknown**: not established; stated explicitly, never guessed.

## Two products, two engines

```
OpenWorker Desktop App (shipped 0.3.1)        OpenWorker Harness SDK (beta 0.1.0-beta.2)
Tauri shell (Rust)  ──spawns──►  Python       Your app ──► Rust runtime (in-process or child)
openworker-desktop               sidecar      agent loop, tools, MCP, approvals, SQLite store,
(updater, autostart,             (PyInstaller) sub-agents, teams, scheduling, budgets
 single-instance, STT)           coworker/*
```

DigitsNepal embeds the **Harness SDK** (Rust). The Desktop App's Python engine
is a design reference only.

## Capability index

| # | Capability | File | Desktop App | Harness SDK |
|---|---|---|---|---|
| 1 | Agent execution loop | [01](01-agent-loop-streaming-cancellation.md) | `coworker/engine.py` TurnEngine | `runtime/src/runtime.rs` |
| 5 | Streaming responses | [01](01-agent-loop-streaming-cancellation.md) | `events.py` EventType | `RunEvent` stream |
| 16 | Error handling and cancellation | [01](01-agent-loop-streaming-cancellation.md) | `request_interrupt` | `Session::cancel`, `RunStatus` |
| 2 | Model provider abstraction | [02](02-providers-and-observability.md) | `providers/base.py` ProviderClient | `ModelProvider` (3 APIs) |
| 17 | Observability and token usage | [02](02-providers-and-observability.md) | `observability/*`, `TokenUsage` | `budget.rs`, `observability` crate |
| 3 | Tool calling and registry | [03](03-tools-and-mcp.md) | `tools/registry.py`, `risk.py` | `tools-sdk` ToolRegistry |
| 4 | MCP integration | [03](03-tools-and-mcp.md) | `mcp/client.py` | `McpServer` stdio / HTTP |
| 6 | Durable sessions | [04](04-sessions-recovery-memory.md) | `conversations.py` SQLite + JSONL | `store.rs` SQLite |
| 7 | Session recovery | [04](04-sessions-recovery-memory.md) | `TurnEngine.resume` | `Harness::resume`, `resume_streamed` |
| 13 | Memory | [04](04-sessions-recovery-memory.md) | `memory/sqlite_store.py` | `assistant_memories`, `memory_scope` |
| 8 | Human approvals | [05](05-approvals-permissions-audit.md) | PermissionRequest + Inbox | `Interruption::Approval` |
| 9 | Permission management | [05](05-approvals-permissions-audit.md) | `permissions.py` modes, floors | `permissions.rs` Mode, Category |
| 14 | Audit logging | [05](05-approvals-permissions-audit.md) | `audit.py` SQLite | events journal, compliance |
| 10 | Sub-agents | [06](06-subagents-teams-automation.md) | `tools/subagent.py` (explore) | `subagents.rs` Config |
| 11 | Multi-agent teams | [06](06-subagents-teams-automation.md) | `teams/*` | `agent_team*.rs` |
| 12 | Scheduling and automations | [06](06-subagents-teams-automation.md) | `automation/scheduler.py` | `schedule_*` built-in tools |
| 15 | Sandbox and isolation | [07](07-sandbox.md) | Seatbelt, OpenShell, Windows | OpenShell or Off |

## Key findings for DigitsNepal

1. **No macOS sandbox in the Harness SDK.** Its backends are OpenShell (Linux
   container) or Off; `direct_macos.rs` is "lifecycle management, not
   containment". The Seatbelt sandbox exists only in the Python Desktop App.
   DigitsNepal must build its own Seatbelt runner before exposing any command
   or untrusted-code execution (Phase 6 requirement).
2. **Approvals are layered**: hard floors → mode → standing rules → optional
   LLM reviewer with a circuit breaker (`_REVIEWER_TRIP = 5` consecutive
   denials) → human. The reviewer can only turn "ask" into "allow", never
   "blocked" into "allow".
3. **Durable wait queue**: every pending prompt (approval, question, folder
   request) is stored so it survives restarts; unattended runs park prompts in
   an Inbox instead of self-approving.
4. **Restart-safe tool calls**: on restart, unanswered trailing tool calls are
   re-processed; answered ones are never executed twice.
5. **Risk classes, not tool names**: READ, EGRESS, WRITE_LOCAL, EXEC, EXTERNAL
   drive gating (Desktop `risk.py`, Harness `permissions::Category`).
6. **Shipped app is not App-Sandboxed**: entitlements are only microphone and
   `disable-library-validation`; it is Developer-ID signed and notarized
   (Observed, REA).

## Phase 0 baseline

See [PHASE0_BASELINE.md](PHASE0_BASELINE.md).
