# DigitsNepal autonomous task agent — assessment and first slice

This answers the "master build prompt" for a Nepali-first autonomous agent:
what exists, what is missing, the order to build it, and what the first
vertical slice (this change) delivers. It is honest about limits: DigitsNepal
does **not** book, buy, pay, send messages or submit applications today.

## 1. Current architecture (inspected)

| Part | Technology | Notes |
|---|---|---|
| Desktop app | Tauri 2 (Rust) + React 18 / TypeScript / Tailwind | macOS, Apple Silicon; shipped as a DMG |
| Translation | Rust engine, provider abstraction (OpenAI-compatible, Gemini, local) | Keys in macOS Keychain |
| Agent engine | OpenWorker Harness SDK (embedded, pinned beta) | Durable sessions, approvals, streaming, limits, compaction |
| Governance store | SQLite (WAL) "ledger" | Grants, sessions, messages, hash-chained audit log, approvals, usage, automations, tasks |
| Policy | Deterministic Rust (`coworker/policy.rs`) | Risk classes; model cannot change grants or policy |
| Tools | Typed registry (`coworker/tools.rs`) | Files, translation, web, GitHub, research, documents, automations, sandboxed code, MCP apps, tasks |
| Sandbox | macOS Seatbelt (`coworker/sandbox.rs`) | For `code.run` only |
| iPhone | PWA + Cloudflare Worker (`pwa/`, `worker/`) | Translation only, access-code auth |

## 2. Spec vs. reality

| Spec item | Status after this change |
|---|---|
| Nepali / Romanized / English understanding | **Done (offline)**: `intent.rs` — category, actions, sensitive actions, dates (भोलि/bholi/weekdays → ISO), times, money (5 hajar, रु ५,०००, 2 lakh, $50), places, preferences. The model refines; extracted values are reference values. |
| Structured task schema + validation | **Done**: `task.plan` / `task.update` / `task.complete` with strict JSON schemas, validated again in Rust |
| State machine (12 states) + durable state | **Done**: `tasks.rs`, transitions validated, compare-and-set in SQLite, every transition logged |
| Survive restarts | **Done**: open approvals persist; interrupted tasks become BLOCKED → Resume |
| Pause / resume / cancel / retry | **Done** |
| Idempotency | **Done for local writes**: an identical successful write in the same conversation is not repeated |
| Verification of results | **Done**: claims checked against the audit log and disk; unverifiable claims (e.g. a "booking") block completion |
| Never change amounts/dates/places | **Done**: plans that drop or change them get warnings, shown to the user |
| Three vertical workflows | **Done as playbooks** on shared tools (local service, job search, supplier deal), each with an explicit stop point |
| Research with sources | **Done**: web search/pages + cited source packs; evidence shown per task |
| Approval engine, standing permissions | **Done** (Phases 3–6). Standing permissions are per chat and revocable; **no expiry time yet** |
| Tamper-evident activity history | **Done** (hash-chained audit log + per-task history) |
| Task dashboard | **Done**: Tasks page (input, live understanding, plan, steps, approvals, sources, verified result, history, metrics) |
| Metrics | **Partly**: completion rate, verified rate, decisions per task, tokens per task, median time. No satisfaction rating yet |
| Communication (email) | **Missing**: needs Gmail OAuth; today the agent can only draft |
| Booking / purchasing / payments | **Not supported** — and stays approval-only when added |
| Browser automation (forms, Playwright) | **Missing**: read-only page fetching only |
| Calendar / scheduling | **Missing** (needs an OAuth calendar integration) |
| Voice (Nepali STT/TTS) | **Missing**: needs a provider choice and a Nepali benchmark |
| User memory (preferences) | **Partly**: language and provider settings; no preference memory yet |
| PostgreSQL, multi-user, cloud workers | **Not adopted** — see §4 |

## 3. Recommended order (next)

1. Run the vertical slice against your real provider and fix prompt issues
   (needs your approval: it spends tokens).
2. Communication: Gmail OAuth read + draft into Gmail Drafts (no send);
   sending as a separate, always-asked action with idempotency keys.
3. Standing permissions with scope + expiry.
4. Preference memory with view/delete controls.
5. Browser automation (Playwright in the sandbox) for form filling, stopping
   before submit.
6. Calendar integration; voice input with a Nepali benchmark.

## 4. Stack decisions

- **SQLite, not PostgreSQL.** DigitsNepal is a single-user desktop app; data
  stays on the Mac. SQLite (WAL, compare-and-set transitions) gives durable
  task state without a server. PostgreSQL becomes relevant only for a hosted,
  multi-user service — that would be a different product with its own
  security review.
- **No Temporal / queues.** The Harness engine already persists sessions and
  pauses on approvals; tasks are a state machine on top of it.
- **No new dependencies or paid services** were added in this slice.

## 5. Operating cost

Only model tokens. Per task: typically 10–40 model calls (limit 40 per turn,
120k output tokens). Web search uses DuckDuckGo's free HTML page; GitHub's
unauthenticated API (60 requests/hour). The Tasks page shows tokens per task.

## 6. Security risks

| Risk | Mitigation |
|---|---|
| Prompt injection from pages, files, apps | Results marked untrusted; policy decides every call; approvals show exact arguments |
| False success claims | `task.complete` verified against the audit log and disk |
| Duplicate side effects on retry | Idempotency for local writes; no external side-effect tools exist |
| Silent value changes in translation | Plan checked against extracted amounts/dates/places |
| Network abuse / SSRF | Public addresses only, DNS pinned, redirects re-checked |
| Running code | Off by default; Seatbelt sandbox; asks every time |
| Cross-user data | Single-user desktop; not applicable until a hosted version exists |

## 7. Testing

- Unit: intent (three languages → one intent, dates, money, places), state
  machine, plan checks, claim verification.
- End to end with the real engine and a scripted model (no paid APIs):
  job-search workflow to a verified result; an invented "booking" blocked;
  restart in the middle of an approval → recover → resume; pause; duplicate
  write not repeated.
- **Not yet done**: a run against a real model provider (costs tokens; needs
  your go-ahead).
