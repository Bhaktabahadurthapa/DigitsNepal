# Phase 0 baseline (2026-11-16)

Branch for this work: `coworker/openworker-investigation`, created from
`coworker/foundation` @ `7e050c4` (open PR #1). `main` is untouched.

## Existing application inventory

| Area | Location | Notes |
|---|---|---|
| Translation engine | `src-tauri/src/translation/*` | detect, lexicon, prompt, validate, engine, 4 protocols, 12 provider presets |
| Provider settings and keys | `translation/registry.rs`, `security/keychain.rs` | per-provider config and cloud consent; keys only in macOS Keychain |
| Selected-text capture | `src-tauri/src/accessibility/*` | AX read/replace, password fields refused |
| Storage | `storage/settings.rs`, `storage/history.rs` | JSON settings; opt-in history |
| Sahayogi coworker | `src-tauri/src/coworker/*` | Harness SDK embedded; policy, ledger (hash-chained audit), scoped files, relay |
| Tauri commands | `src-tauri/src/commands/*` | the only webview ↔ native bridge |
| UI | `src/app`, `src/features/*` | Translate, Word Explorer, History, Sahayogi, Activity, Settings |
| iPhone web app and API | `pwa/`, `worker/` | Cloudflare Worker |

## Baseline test results (all passing)

| Suite | Result |
|---|---|
| TypeScript typecheck | pass |
| Frontend (Vitest) | 17 passed |
| Evaluation dataset check | pass |
| Rust fmt / clippy `-D warnings` | pass / pass |
| Rust unit tests | 97 passed |
| Coworker end-to-end (real Harness + scripted model) | 9 passed |
| Language-detection eval | 1 passed |
| Cloudflare Worker | 18 passed |
| iPhone web app | 14 passed |

No application code changes were made in Phase 0 or Phase 1.
