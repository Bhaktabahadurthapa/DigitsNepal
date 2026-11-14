# Architecture

DigitsNepal is a Tauri 2 app: a Rust core with a React/TypeScript webview UI.

```
┌───────────────────────── Webview (React, no privileges) ─────────────────────────┐
│ Translate · Live Compose · Word Explorer · History · Settings · Quick popover     │
│ src/providers/api.ts — typed invoke() wrappers, the only path to native code      │
└───────────────────────────────────┬──────────────────────────────────────────────┘
                                    │ Tauri IPC (app commands only)
┌───────────────────────────────────▼──────────────────────────────────────────────┐
│ src-tauri/src/commands      — validates every input, maps errors to {code,msg}   │
│ src-tauri/src/translation   — engine (pure, provider-agnostic)                    │
│     detect.rs   offline language detection (script + lexicon + suffixes)          │
│     lexicon.rs  curated Romanized-Nepali lexicon (detection + offline glosses)    │
│     prompt.rs   mode-specific prompts, fenced <source> data block                 │
│     validate.rs untrusted-model-output validation + faithfulness checks           │
│     engine.rs   input limits, retry policy, no fabrication on failure             │
│     provider.rs trait Provider { list_models, complete_json }                     │
│     registry.rs presets (12), endpoint rules, consent + key checks, factory       │
│     ollama.rs / gemini.rs / anthropic.rs / openai.rs (OpenAI-compatible)          │
│ src-tauri/src/accessibility — AX selected-text read / replace / undo + safety.rs  │
│ src-tauri/src/shortcuts     — accelerator validation, reserved-combo list         │
│ src-tauri/src/clipboard     — explicit-action-only clipboard                      │
│ src-tauri/src/security      — Keychain (one entry per provider)                   │
│ src-tauri/src/storage       — settings.json, opt-in history.json (0600, atomic)   │
│ src-tauri/src/lib.rs        — app setup, tray menu, global shortcut, windows      │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Translation flow

1. `engine::check_input` trims and rejects empty text and text over 6,000 characters.
2. `detect::detect` makes an offline language guess, which is passed to the model as a *hint* only.
3. `prompt::build` combines the core rules, the mode instructions and the JSON schema. User text sits inside `<source>…</source>`, and any `</source>` in the input is neutralised.
4. `Provider::complete_json` calls the selected provider in JSON mode: Ollama `format: json`, Gemini `responseMimeType`, OpenAI-compatible `response_format: json_object` (retried without optional params if a server rejects them), Anthropic (prompted JSON). The validator enforces JSON either way.
5. `validate::validate` extracts the JSON object, requires `accurate_english`, overrides `original_text` with the real input, drops malformed optional fields, flags word meanings whose `source` isn't in the input, ignores model "confidence", and adds deterministic warnings (lost negation, missing numbers, length mismatch).
6. Retry policy: one retry for transient errors (network, timeout, 5xx, short rate limits), and one "repair" retry for malformed JSON. No retries for auth or model-missing errors. On failure the user gets an error and their text stays untouched.

The faithful translation (`accurate_english`) and the styled rewrite (`styled_english`) are always separate fields. Accurate mode returns a Professional rendering in `styled_english` for side-by-side comparison. Grammar mode never returns one.

## Quick-translate (shortcut) flow

1. When the global shortcut fires, `accessibility::capture()` runs **before** our window takes focus. It reads `AXFocusedUIElement → AXSelectedText` and `AXSelectedTextRange`, and keeps a retained reference to the element and the app's pid.
2. The capture is refused when Accessibility isn't trusted, when secure event input is active, or when the focused element is an `AXSecureTextField`.
3. The popover shows and translates. **Replace** hides the popover, re-activates the source app (`AXFrontmost`), checks that the current selection still equals the captured text, then sets `AXSelectedText`.
4. **Undo** (menu bar) checks that the inserted text is still at the recorded range (`AXStringForRange`), reselects it, and writes the original back. If the text has changed since, undo is refused.
5. If the app doesn't expose selected text, the popover says so. It offers *Use clipboard text* only if the user enabled the fallback, and only reads the clipboard when they click.

## Providers

| Preset | Protocol | Local? | Key |
|---|---|---|---|
| Ollama | Ollama | ✅ | — |
| LM Studio | OpenAI-compatible | ✅ | — |
| Google Gemini | Gemini | cloud | required |
| Anthropic Claude | Anthropic Messages | cloud | required |
| OpenAI, OpenRouter, Groq, Mistral, DeepSeek, Together, xAI | OpenAI-compatible | cloud | required |
| Custom | OpenAI-compatible, any `/v1` base URL | local if loopback, else cloud | optional |

**Adding a service that speaks the OpenAI API** takes one line in `registry::PRESETS`. **Adding a new protocol** means implementing `translation::provider::Provider` (two async methods), adding a `Protocol` variant, and wiring it in `registry::build`. Prompts, validation and retries are shared. Add transport tests the way `openai.rs` does (it uses a one-shot local HTTP server).
