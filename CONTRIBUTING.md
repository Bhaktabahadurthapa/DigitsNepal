# Contributing

Thanks for helping DigitsNepal! Every Nepali word has meaning, and so does every contribution.

## Setup

```bash
# Prerequisites: Xcode Command Line Tools, Rust (rustup), Node 20+
npm install
npm run app:dev        # runs the desktop app with hot reload
```

## Before opening a PR

```bash
npm run check          # typecheck, vitest, eval dataset check, rustfmt, clippy, cargo test
```

## Good first contributions

* **Lexicon** (`src-tauri/src/translation/lexicon.rs`): add common Romanized spellings with Devanagari and a short gloss. Mark ambiguous spellings `ambiguous`.
* **Evaluation cases** (`scripts/build-eval-dataset.py`, then regenerate the JSON): add realistic sentences with a careful human reference translation and notes.
* **App compatibility reports**: tell us which apps do or don't support selected-text read and replace.

## Principles

* Never fabricate output. If a model fails, show an error and keep the user's text.
* Keep faithful translation and stylistic rewriting separate.
* Privacy first: no new network calls, telemetry, or background access without an explicit, documented user opt-in.
* Translation logic stays in `translation/`, separate from the UI and from OS integration.
* Don't claim a feature works on macOS until you've verified it by hand. Record what you tested in the PR.
