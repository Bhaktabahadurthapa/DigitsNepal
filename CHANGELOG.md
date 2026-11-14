# Changelog

## 0.1.0 — 2026-11-14 (development release, unsigned)

### Added
- Translation engine with any AI provider: Ollama and LM Studio (local); Gemini, Anthropic Claude, OpenAI, OpenRouter, Groq, Mistral, DeepSeek, Together, xAI (cloud); or any custom OpenAI-compatible endpoint. Model discovery, per-provider Keychain keys and consent, and structured, validated JSON output.
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

### Not yet implemented
- Voice input (Phase 4).
- System-wide inline live typing (see docs/LIVE_TYPING.md).
- Code signing and notarization.
