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

## Specialists (Phase 4)

A new chat starts by choosing a specialist. Each one has its own role and only
the tools that role needs; the policy engine still decides every call, so a
specialist can never do more than its tool list allows. A model that names a
tool its specialist was not given gets nothing (tested end to end).

| Specialist | Role | Tools |
|---|---|---|
| Sahayogi (सहयोगी) | General coworker | All local tools |
| Lekhak (लेखक) | Nepali–English writer: translate, polish, draft | Translation, read files, create file |
| Khoji (खोजी) | Research from your own documents, with citations | Translation, read files (read-only) |
| Kagaj (कागज) | Create and organize documents | Translation, read, create, move |
| Prabidhi (प्रविधि) | Explain, review and draft code | Explain, read, create (cannot run code) |
| Swachalit (स्वचालित) | Plan repeatable workflows | Read only (cannot run or schedule yet) |

Each specialist is stored as its own immutable, versioned definition
(`dn-<key>-v<version>-<digest>`). Web search, GitHub and running automations
arrive with the Phase 5 tools; until then those specialists say so instead of
pretending.

## Tools (Phase 5)

| Tool | Risk class | What it does | Limits |
|---|---|---|---|
| `browser.search` | egress (asks) | DuckDuckGo results: title, link, snippet | 10 results, 30 s |
| `browser.open` | egress (asks) | Readable text of a public page | http/https only; 2 MB; 5 redirects |
| `research.summarize` | read | Extractive, cited source pack (S1, S2…) from files and text | 10 sources; paths must be granted |
| `documents.create` | write_local (asks) | Titled Markdown / HTML / text document | extension must match format; backs up |
| `github.search` | egress (asks) | Public repositories or issues/PRs | read-only, unauthenticated |
| `github.inspect` | egress (asks) | Repo summary + README, or a path's listing/text | `owner/name` validated |
| `automation.create` | asks | Save a named list of ≤ 20 tool steps | steps validated; no nesting |
| `automation.list` | read | Saved automations | — |
| `automation.run` | asks per run | Run a saved automation | each step re-checked; stops at first failure |

**Network safety.** Every request resolves the host itself, refuses
private, loopback, link-local, carrier-NAT and other non-public addresses
(IPv4 and IPv6, including IPv4-mapped), pins the connection to the checked
address so DNS cannot be switched mid-request, and re-checks every redirect.
Results are marked `untrusted_content`; the instructions tell every specialist
to treat them as data. Search queries go to DuckDuckGo only after approval.

**Automations.** Steps may use only DigitsNepal's typed file, document,
translation, research, browser and GitHub tools — never commands, connected
apps, other automations, or anything on the hard-floor list. Running one asks
you with every step listed (an automation whose steps only read inside granted
folders runs without asking). Each step is checked again by the policy at run
time, so a revoked folder stops the run. Automations run only when started
from a chat; there is no scheduler yet. Delete them under **Tools & Apps**.

**Connected apps (MCP).** Add a remote MCP server (https, or http on this Mac)
under **Tools & Apps**. DigitsNepal's own MCP client lists its tools; every
tool starts **off** and you switch on the ones Sahayogi may use. Enabled tools
go to the general Sahayogi only. Server annotations can only make a call
stricter: "read-only" tools ask (and may be allowed for the chat),
"destructive" tools are hard-floor, everything else asks every time, and
names like `send_email` stay hard-floor whatever the server claims. Changing
connected apps restarts the engine, so it is refused while a chat is running.
Not yet supported: servers that need a login token, and local MCP programs
(they would run code outside a sandbox).

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
