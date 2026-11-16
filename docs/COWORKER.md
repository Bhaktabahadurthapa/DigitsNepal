# Sahayogi (सहयोगी): DigitsNepal AI work assistant (preview)

Sahayogi is the AI work assistant inside the DigitsNepal Mac app. You can ask
for work in Romanized Nepali, Devanagari or English, and it works only in
folders you grant.

## Architecture

```
React UI: Sahayogi · Activity
        │  Tauri commands + "coworker:event" stream
Rust  ──┼─ coworker::service ── OpenWorker Harness SDK (embedded, pinned commit, MIT)
        │      every tool call pauses (Approvals::Always) → coworker::policy decides
        ├─ coworker::policy   allow / ask / deny; hard floors; target authorization
        ├─ coworker::ledger   SQLite: folder grants, sessions, messages,
        │                     hash-chained audit log
        ├─ coworker::files    folder-scoped list/search/read/PDF/write (copy-on-write)
        ├─ coworker::language offline Nepali detection + dictionary glosses
        └─ coworker::providers  Settings + Keychain → engine model connection
Sidecar: openworker-harness (engine runner; Contents/MacOS)
```

- **One agent engine.** The OpenWorker Harness runs in-process. DigitsNepal's existing translation engine is untouched.
- **One authority.** The engine pauses before every tool call, and `policy::evaluate` answers each pause. The model has no tool that changes grants or policy.
- **Models.** The coworker uses the provider chosen in Settings → AI provider: Ollama, LM Studio, Gemini, Anthropic, or any OpenAI-compatible provider. Cloud consent and the Keychain key are required as before.

## Agent engine (Phase 3)

**Typed tools** (`coworker/tools.rs`): every tool declares a risk class, a
strict input schema (`additionalProperties: false`), an output schema, a
timeout and whether it can be cancelled. Each run is wrapped with the timeout
and the engine's cancellation token and recorded as a `tool_run` audit entry.

| Tool | Risk | Timeout |
|---|---|---|
| `translation.translate` (existing translation engine) | read | 90 s |
| `translation.explain` (offline detection + dictionary) | read | 5 s |
| `files.granted_folders`, `files.list`, `files.search`, `files.read`, `files.read_pdf` | read | 5–45 s |
| `files.create` (backs up an existing file) | write_local | 20 s |
| `files.move` (never overwrites) | write_local | 20 s |

There are no command, delete or "external" tools. The policy decides by risk
class: reads inside granted folders run; writes ask; command tools are refused
until the macOS sandbox exists; external actions are always-ask hard floors.
Legacy `dn.*` names stay registered so older conversations keep working.

**Agent definitions** (`coworker/agents.rs`): versioned and content-addressed
(`dn-coworker-v2-<digest>`). Stored once in the ledger and never modified; a
conversation always resumes with the definition it started with.

**Restart-safe approvals**: open approvals and questions are stored in
`pending_interactions`. After a restart, opening the conversation shows them
again; answered calls are never re-run.

**Limits and context**: 40 model calls and 120,000 output tokens per turn;
history is summarised automatically at 80 % of the model's context window.
If the engine cannot tell whether a request finished, it is not retried.

**Token usage**: recorded per model call from the engine's `model_usage`
events (`usage` table) and shown as a token count in the chat header.

## What runs automatically, what asks, what is refused

| Action | Decision |
|---|---|
| Understand Nepali text (offline) | Runs |
| List, search or read files inside a granted folder | Runs |
| Read outside granted folders, or credential files (`.env`, keys, `.ssh`…) | Refused |
| Write or move a file inside a granted folder | Asks; an existing file is backed up first; moves never overwrite |
| Send, delete, install, publish, push, purchase, submit or deploy (any tool) | Always asks, once only |
| Unknown tools | Asks |

There is no delete tool. Stopping a turn denies its pending approvals. Unanswered approvals never auto-approve.

## Activity log

Every decision is recorded with who decided (policy or you), the tool, and a
SHA-256 fingerprint of its arguments. File contents are not stored. Each entry
includes the hash of the previous one, so editing or deleting an entry fails
the integrity check shown on the Activity screen.


## Building

```bash
scripts/build-harness-runner.sh     # builds the engine runner sidecar (pinned commit)
npm run app:build
```

Tests:

```bash
DN_HARNESS_RUNNER=src-tauri/binaries/openworker-harness-$(rustc -vV | sed -n 's/^host: //p') \
  cargo test --manifest-path src-tauri/Cargo.toml --features coworker
```

The engine tests run the real Harness against a scripted local model server,
so they make no paid API calls.

## Known limitations (MVP)

- One specialist (General) for now. Browser automation, terminal, automations, voice and iPhone are later phases.
- The coworker has not been tried with a live cloud model yet: all engine tests use a scripted local model.
- OpenWorker's Harness SDK is beta (0.1.0-beta.2) and pinned to commit `ecb4454`. Upgrades need re-testing.
- The engine runner adds about 25 MB to the app.
