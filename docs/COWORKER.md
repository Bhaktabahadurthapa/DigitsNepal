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

## What runs automatically, what asks, what is refused

| Action | Decision |
|---|---|
| Understand Nepali text (offline) | Runs |
| List, search or read files inside a granted folder | Runs |
| Read outside granted folders, or credential files (`.env`, keys, `.ssh`…) | Refused |
| Write a file inside a granted folder | Asks; the existing file is backed up first |
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
