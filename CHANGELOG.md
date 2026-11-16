# Changelog

## Unreleased

### Added
- Sahayogi (सहयोगी), the AI work assistant (preview): understands Romanized Nepali, works only in folders you grant, asks before writing, and records every decision in a tamper-evident Activity log. Powered by the OpenWorker Harness SDK (embedded).
- iPhone web app at digitsnepal.com/app: an installable PWA with Accurate, Professional and Grammar modes, word meanings, one-tap copy and share, light and dark appearance, and optional on-device history (off by default).
- Private translation API on Cloudflare Workers: access-code sign-in with signed device tokens, origin lock, daily quota, burst and size limits, automatic Gemini model selection, and no content logging.

## 0.1.0 — 2026-11-14 (development release, unsigned)

### Added
- DigitsNepalAI translation engine with any model provider: Ollama and LM Studio (local); Gemini, Anthropic Claude, OpenAI, OpenRouter, Groq, Mistral, DeepSeek, Together, xAI (cloud); or any custom OpenAI-compatible endpoint. Model discovery, per-provider Keychain keys and consent, and structured, validated JSON output.
- Six modes: Accurate, Professional, Casual, Academic, Technical, Grammar Correction.
- Offline language detection (Romanized, Devanagari, English, mixed) and a curated lexicon.
- Side-by-side workspace: original, Devanagari, accurate English, styled English, warnings, ambiguities.
- Word Meaning Explorer with clickable words and phrases, contextual (model) and dictionary (offline) glosses, and uncertainty flags.
- Global shortcut (default ⌃⌥T) → selected-text capture through Accessibility → quick-translate popover with Copy, Replace, Edit and Cancel; undo the last replacement from the menu bar.
- Menu bar item: open, translate clipboard text, mode, pause/resume, settings, quit.
- Opt-in local history: search, favorites, delete, clear all, JSON export, per-translation private mode.
- Live auto-translate while typing on the Translate screen (on by default; pausable), and a Keep-on-top pin for meetings.
- Keychain storage for API keys; loopback-only Ollama; cloud consent gate.
- 111-case evaluation dataset, offline detection test, and live eval runner.

- Overlay mode: compact always-on-top window on all Spaces, adjustable background opacity (text stays solid), and "Hide from screen sharing" (macOS content protection) for the main window and popover.

### Not yet implemented
- Voice input (Phase 4).
- System-wide inline live typing (see docs/LIVE_TYPING.md).
- Code signing and notarization.
