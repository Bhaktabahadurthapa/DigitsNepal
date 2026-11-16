# DigitsNepal Coworker (MVP)

An AI coworker inside the DigitsNepal Mac app. You can ask it for work in
Romanized Nepali, Devanagari or English, and it works only in folders you
grant. It can also investigate websites and apps you are authorized to inspect,
using [REA](https://github.com/morluto/rea).

## Architecture

```
React UI: Coworker · REA Lab · Activity
        │  Tauri commands + "coworker:event" stream
Rust  ──┼─ coworker::service ── OpenWorker Harness SDK (embedded, pinned commit, MIT)
        │      every tool call pauses (Approvals::Always) → coworker::policy decides
        ├─ coworker::policy   allow / ask / deny; hard floors; target authorization
        ├─ coworker::ledger   SQLite: folder grants, REA targets, sessions, messages,
        │                     hash-chained audit log
        ├─ coworker::files    folder-scoped list/search/read/PDF/write (copy-on-write)
        ├─ coworker::language offline Nepali detection + dictionary glosses
        ├─ coworker::providers  Settings + Keychain → engine model connection
        └─ coworker::rea      Node/REA discovery, scoped stdio MCP, live tool catalog
Sidecar: openworker-harness (engine runner; Contents/MacOS)
```

- **One agent engine.** The OpenWorker Harness runs in-process. DigitsNepal's existing translation engine is untouched.
- **One authority.** The engine pauses before every tool call, and `policy::evaluate` answers each pause. The model has no tool that changes grants, targets or policy.
- **Models.** The coworker uses the provider chosen in Settings → AI provider: Ollama, LM Studio, Gemini, Anthropic, or any OpenAI-compatible provider. Cloud consent and the Keychain key are required as before.

## What runs automatically, what asks, what is refused

| Action | Decision |
|---|---|
| Understand Nepali text (offline) | Runs |
| List, search or read files inside a granted folder | Runs |
| Read outside granted folders, or credential files (`.env`, keys, `.ssh`…) | Refused |
| Write a file inside a granted folder | Asks; the existing file is backed up first |
| Send, delete, install, publish, push, purchase, submit or deploy (any tool) | Always asks, once only |
| REA on a target that is not authorized | Refused |
| REA steps that launch, drive or instrument a target | Always asks, once only |
| Other REA steps on an authorized target | Asks (runs only if REA marks it read-only) |
| Unknown tools | Asks |

There is no delete tool. Stopping a turn denies its pending approvals. Unanswered approvals never auto-approve.

## Activity log

Every decision is recorded with who decided (policy or you), the tool, and a
SHA-256 fingerprint of its arguments. File contents are not stored. Each entry
includes the hash of the previous one, so editing or deleting an entry fails
the integrity check shown on the Activity screen.

## REA Lab

1. Install REA 6.8.0 or newer (`npm install -g rea-agents@latest`) and Node.js 22.23.4+, 24.22.1+ or 26.12.1+.
2. In REA Lab, choose a type (Website, JavaScript/Electron app, or Desktop app), enter the target, and state your authorization.
3. DigitsNepal starts the official `rea mcp` server, limited to that type's tool families, reads its live tool catalog, and starts the REA specialist.

REA versions before 6.8.0 ignore the family scope and expose about 200 tools, so DigitsNepal refuses them.

## Building

```bash
scripts/build-harness-runner.sh     # builds the engine runner sidecar (pinned commit)
npm run app:build
```

Tests:

```bash
DN_HARNESS_RUNNER=src-tauri/binaries/openworker-harness-$(rustc -vV | sed -n 's/^host: //p') \
  cargo test --manifest-path src-tauri/Cargo.toml --features coworker
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored   # live REA catalog contract test
```

The engine tests run the real Harness against a scripted local model server,
so they make no paid API calls.

## Known limitations (MVP)

- Specialists: General and REA only. Browser automation, terminal, automations, voice and iPhone are later phases.
- The REA website workflow is implemented, but it has not been run end to end on this machine because the installed REA is 6.7.0 (it needs 6.8.0 or newer).
- The coworker has not been tried with a live cloud model yet: all engine tests use a scripted local model.
- OpenWorker's Harness SDK is beta (0.1.0-beta.2) and pinned to commit `ecb4454`. Upgrades need re-testing.
- The engine runner adds about 25 MB to the app.
