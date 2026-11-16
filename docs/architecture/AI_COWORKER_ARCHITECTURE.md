# DigitsNepal AI coworker architecture

One macOS process (Tauri + Rust) with the OpenWorker Harness embedded. No
microservices. The existing translator is unchanged and becomes a tool.

```
┌──────────────────────────── DigitsNepal.app ────────────────────────────┐
│ React UI                                                                │
│  Home · AI Coworker · AI Writer · Research · Documents · Agents ·       │
│  Automations · Integrations · Approval Center · Activity · Settings     │
├───────────── Tauri commands + "coworker:event" stream ──────────────────┤
│ coworker::service      sessions, turns, event forwarding, cancellation  │
│   └─ OpenWorker Harness (in-process, pinned) — loop, store, sub-agents  │
│ coworker::agents       versioned AgentDefinition registry               │
│ coworker::tools        typed ToolSpec registry (risk, schemas, timeout) │
│   ├─ translation.*  → translation::engine (existing)                    │
│   ├─ files.*        → coworker::files (scoped, copy-on-write)           │
│   ├─ browser.* / research.*  → fetch + extract (egress approval)        │
│   ├─ documents.*    → files + templates                                 │
│   ├─ github.*       → GitHub REST with user token (Keychain)            │
│   ├─ automation.*   → coworker::automation                              │
│   └─ mcp.*          → user-added MCP servers (Integrations)             │
│ coworker::policy       allow / ask / deny — the only authority          │
│ coworker::approvals    durable pending queue, standing rules            │
│ coworker::ledger       SQLite: grants, rules, sessions, audit chain…    │
│ coworker::sandbox      Seatbelt runner (required before exec tools)     │
│ coworker::automation   scheduler, task runs, unattended inbox           │
│ coworker::relay        loopback relay for OpenAI-compatible gateways    │
│ translation::*         existing engine, providers, lexicon (unchanged)  │
│ security::keychain     all secrets                                      │
└─────────────────────────────────────────────────────────────────────────┘
        │ model calls (user's provider)      │ GitHub / web (approved egress)
```

## Boundaries

| Boundary | Rule |
|---|---|
| Webview ↔ native | Only Tauri commands; webview never sees keys or raw tool access |
| Model ↔ tools | Model proposes; `policy` decides; handler re-checks scope |
| Engine ↔ keys | Engine gets per-launch relay tokens or memory-only credentials |
| Tools ↔ OS | File tools are scoped; exec only through the Seatbelt runner |
| Agents ↔ agents | Delegation only via the orchestrator, depth ≤ 1, budgets |

## Agents (all share the same tools, policy and ledger)

| Agent | Purpose | Default tools |
|---|---|---|
| DigitsNepal Coworker (Sahayogi) | General work in Nepali/English | translation.*, files.*, documents.*, research.summarize |
| Nepali-English Writer | Translate, rewrite, grammar, explain | translation.* only |
| Research Agent | Web + document research with references | browser.search/open, research.summarize, files.read |
| Document Agent | Read, organise, summarise, create documents | files.*, documents.create |
| Technical Agent | Code, debugging, DevOps | files.*, github.*, (exec only after sandbox) |
| Automation Agent | Create and run scheduled workflows | automation.*, read tools |

Agent definitions are versioned and content-digested; a session stays bound
to the exact definition it started with.

## Delivery order

1. Typed tool registry + agent definitions + persisted approvals (Phase 3–5 core).
2. Writer, Document, Research agents (no exec needed).
3. Approval Center, Activity, Integrations (Phase 6 UI).
4. Seatbelt runner, then Technical Agent exec tools.
5. Automations, then orchestrator and sub-agents (Phase 7).
