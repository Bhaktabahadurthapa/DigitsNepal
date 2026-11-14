<p align="center"><img src="src-tauri/icons/128x128@2x.png" width="96" alt="DigitsNepal icon"></p>

# DigitsNepal

**Every Nepali Word Has Meaning.**

DigitsNepal is a free, open-source, privacy-first macOS app for writing and translating between Nepali and English. Type Romanized Nepali the way you normally do:

> Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.

and get natural English, without leaving the app you're writing in:

> I discussed the technical problem during yesterday's meeting, but the solution was not clear.

It works with **any AI provider**. Run a model on your own Mac for free with Ollama or LM Studio, or bring your own key for Gemini, Claude, OpenAI, OpenRouter, Groq, Mistral, DeepSeek, Together, xAI, or any OpenAI-compatible server.

## Features

| | Status |
|---|---|
| **Translate workspace**: original, Devanagari, faithful English and a polished rewrite side by side, each copyable | ✅ |
| **Six modes**: Accurate (default), Professional, Casual, Academic, Technical, Grammar Correction | ✅ |
| **Word Meaning Explorer**: click any Nepali word or phrase for its contextual meaning, literal meaning, Devanagari spelling and uncertainty flags. Offline dictionary glosses work with no model at all | ✅ |
| **System-wide shortcut** (default ⌃⌥T): select text in another app, then translate, copy, **replace** or edit it in a compact popover. Undo the last replacement from the menu bar | ✅ built; see [verification status](#verification-status) |
| **Menu bar**: open, translate clipboard text, mode, pause/resume, settings, quit | ✅ |
| **Smart language detection** (offline): Romanized Nepali, Devanagari, English, mixed | ✅ |
| **Live Compose**: a dedicated window that translates as you pause or finish a sentence | ✅ (opt-in) |
| **History** (local, opt-in): search, favorites, delete, clear all, JSON export, private mode | ✅ |
| **Any AI provider**, local or cloud, with model discovery | ✅ |
| Voice input | ⏳ planned (Phase 4) |
| Inline live typing in every app (input method) | ⏳ researched, see [docs/LIVE_TYPING.md](docs/LIVE_TYPING.md) |

## Install

### From source (recommended for now)

Requirements: macOS 12+, Xcode Command Line Tools (`xcode-select --install`), [Rust](https://rustup.rs), Node.js 20+.

```bash
git clone https://github.com/Bhaktabahadurthapa/DigitsNepal.git
cd DigitsNepal
npm install
npm run app:dev       # run in development
npm run app:build     # build DigitsNepal.app and a .dmg
```

### From a release DMG

Release builds are currently **unsigned and not notarized**. After dragging DigitsNepal to Applications, right-click it and choose **Open** the first time. Apple Silicon and Intel builds are produced by CI. See [docs/RELEASE.md](docs/RELEASE.md).

## Choose an AI provider

Open **Settings › AI provider**:

| Provider | Cost | Privacy | Setup |
|---|---|---|---|
| **Ollama** (default) | Free | Text stays on your Mac | Install [Ollama](https://ollama.com), then `ollama pull gemma3` (or another multilingual model) |
| **LM Studio** | Free | Text stays on your Mac | Start LM Studio's local server |
| **Gemini · Claude · OpenAI · OpenRouter · Groq · Mistral · DeepSeek · Together · xAI** | Your API account | Text is sent to that provider | Paste your API key (stored in macOS Keychain), confirm cloud use, pick a model |
| **Custom** | — | Local if `localhost`, cloud otherwise | Any OpenAI-compatible `/v1` URL (vLLM, llama.cpp server, gateways), with an optional key |

Model lists are fetched live from each provider. Nothing is hard-coded, and you can always type a model ID yourself.

> Translation quality varies a lot between models, especially for Romanized Nepali. Use the [evaluation set](docs/EVALUATION.md) to compare models before relying on one.

## Use the shortcut in any app

1. Settings › *Keyboard shortcut & selected text* › **Open System Settings**, and allow DigitsNepal under *Accessibility*.
2. Select text in any app and press **⌃⌥T**.
3. In the popover: **Replace** puts the English in place of your selection, **Copy** copies it, **Edit** lets you change the original first, and **Esc** cancels and returns you to your app.

Safety rules: DigitsNepal reads only the *selected* text, only when you press the shortcut. It never reads password fields. It replaces text only if your selection hasn't changed, and it never touches the clipboard unless you click Copy or the clearly labelled clipboard option.

## Privacy

There's no account, no telemetry, and no analytics. Your text only goes to the provider you choose, and cloud providers need a separate opt-in for each provider. API keys live in the macOS Keychain. Details: [docs/PRIVACY.md](docs/PRIVACY.md) · threat model: [SECURITY.md](SECURITY.md).

## Verification status

What was verified while building v0.1.0 (macOS 27, Apple Silicon):

* ✅ `npm run check`: TypeScript, Vitest, dataset validation, rustfmt, clippy `-D warnings`, and all Rust tests pass (engine, validation, providers against local mock servers, registry, history, settings, shortcuts, AX safety rules).
* ✅ The release `.app` and `.dmg` build and the app launches.
* ✅ The UI flows were checked in a browser preview with demo data.
* ⚠️ **Not yet verified:** live translation against a real model (no model or API key was available on the build machine), and the Accessibility read/replace flow in real third-party apps. App-compatibility reports are very welcome.

## Develop

```bash
npm run check                                  # everything CI runs
cd src-tauri && cargo run --example eval -- --provider ollama --model <model>
```

Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · Contributing: [CONTRIBUTING.md](CONTRIBUTING.md) · Changelog: [CHANGELOG.md](CHANGELOG.md)

## License

MIT. See [LICENSE](LICENSE).
