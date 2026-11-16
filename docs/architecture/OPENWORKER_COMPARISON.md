# OpenWorker vs DigitsNepal: gap analysis

Inputs: [Phase 1 investigation](../reverse-engineering/openworker/README.md) and
DigitsNepal `coworker/foundation` @ `7e050c4`.

Legend: ✅ have · 🟡 partial · ❌ missing.

## Capability matrix

| # | Capability | OpenWorker (reference) | DigitsNepal today | Gap | Plan |
|---|---|---|---|---|---|
| 1 | Agent loop | Desktop TurnEngine; Harness runtime | ✅ Harness embedded (`coworker/service.rs`) | Execution limits not configured | Set `TurnLimits` per agent; expose in UI |
| 2 | Provider abstraction | ProviderClient; Harness 3 APIs | ✅ 12 presets mapped by `coworker/providers.rs`; relay for gateway quirks | Per-agent model choice | Model selection per agent definition |
| 3 | Tool registry | ToolSpec + risk classes | 🟡 7 `dn.*` tools; policy by tool name | No declared risk class, output schema, timeout per tool | Typed `ToolSpec` with risk class, schemas, timeout (Phase 5) |
| 4 | MCP | stdio + HTTP, OAuth | 🟡 SDK supports it; not exposed | No user-facing MCP connections | Integrations screen; MCP tools default to "ask" |
| 5 | Streaming | rich event stream | ✅ text, tool, approval, error events | No plan/progress events | Forward `PlanUpdated`, sub-agent activity |
| 6 | Durable sessions | SQLite + JSONL / SQLite | ✅ Harness store + DigitsNepal session index and messages | — | Keep |
| 7 | Recovery | resume unanswered calls | 🟡 engine resume works (tested); pending approvals are in memory | Pending approvals lost on restart | Persist pending interruptions; re-show cards on launch |
| 8 | Approvals | cards, Inbox, reviewer, breaker | 🟡 cards, once / session / deny | No Approval Center, no unattended inbox | Approval Center screen (Phase 6) |
| 9 | Permissions | modes, floors, standing rules | 🟡 folder grants, hard-floor words, standing "session" grants | No modes (read-only), no revocable standing rules list | Modes + rules table + revoke UI |
| 10 | Sub-agents | explore child, depth 1 | ❌ | — | Harness `subagent` with depth 1, budgets (Phase 7) |
| 11 | Teams | board + journal | ❌ | — | Orchestrator only after single-agent is stable (Phase 7) |
| 12 | Automations | cron scheduler, inbox | ❌ | — | Scheduler + task runs + inbox (Automation Agent) |
| 13 | Memory | scoped SQLite | ❌ | — | Opt-in, user-visible memory (later) |
| 14 | Audit | sanitized SQLite log | ✅ hash-chained ledger (stronger) | No redaction of secret-like keys (we store digests only) | Keep digests; add per-tool result summaries |
| 15 | Sandbox | Seatbelt / OpenShell / Windows | ❌ none (SDK has none on macOS) | **Blocker for any exec tool** | DigitsNepal Seatbelt runner before exec tools |
| 16 | Cancellation | from any state | ✅ cancel denies pending approvals (tested) | — | Keep |
| 17 | Observability / tokens | recorder, calculator | ❌ no token/cost view | — | Usage table + per-session token pill |

## Reusable DigitsNepal parts

| Part | Reuse |
|---|---|
| `translation::engine::translate` + `detect` + `lexicon` + `validate` | Backend for `translation.translate` / `translation.explain` tools and the Nepali-English Writer agent. Keeps faithfulness checks. |
| `translation::registry` + `security::keychain` | Single source for model connections; keys never leave Keychain/relay |
| `coworker::policy` | Becomes the policy engine; extend with risk classes and modes |
| `coworker::ledger` | Extend with approvals, rules, usage, automations tables |
| `coworker::files` | Backend for `files.list/read/create/move` (add move with backup) |
| `coworker::relay` | Keep for OpenAI-compatible gateways; hides keys from the engine |
| `accessibility` | Later: "use selected text" as an explicit attachment |

## New modules required

`coworker/tools/` (typed registry), `coworker/agents/` (versioned definitions),
`coworker/approvals.rs` (durable pending queue), `coworker/sandbox/` (Seatbelt
runner), `coworker/automation/` (scheduler), `coworker/usage.rs`,
`coworker/research/` (fetch + extract with egress approval).

## SDK compatibility

- OpenWorker Harness `0.1.0-beta.2`, pinned `ecb4454`; not on crates.io.
- Agent definitions immutable → our agent IDs already include a content digest.
- Requires bundled runner (25 MB) when tools exist.
- Strict stream validation → relay needed for some gateways.
- Risk: API churn; keep the `coworker::service` boundary so the engine can be swapped.

## Performance risks

- App size +25 MB runner; build time +~1 min.
- Engine start on first use (~1 s); keep lazy start.
- PDF extraction via `osascript` per file; batch limits needed for large folders.
- Many MCP tools inflate model context → scope tools per agent.
