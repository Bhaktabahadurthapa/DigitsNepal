# Translation evaluation

`tests/fixtures/eval-dataset.json` holds **111 hand-written cases**. Generate it from `scripts/build-eval-dataset.py`.

| Category | Cases | Covers |
|---|---|---|
| everyday | 15 | greetings, daily life, common idioms |
| workplace | 15 | meetings, deadlines, reports, hedges ("sayad") |
| technical | 20 | Kubernetes, AWS, CI, Git, Docker, versions, error codes |
| support | 5 | customer-support requests and replies |
| academic | 8 | research, theses, hedged claims |
| informal | 10 | slang, very informal pronouns (ta-form) |
| devanagari | 12 | native script, including mixed Devanagari + English |
| tense | 12 | present, past, perfect, progressive, prohibitive, negation |
| ambiguous | 6 | ma (I / in), ko (of / who), idioms |
| grammar | 5 | English grammar-correction mode |
| long | 3 | multi-sentence paragraphs |

Each case has a `reference` (**one** acceptable translation, not the only one), `must_keep` terms that must survive verbatim, `must_negate`, and reviewer `notes`.

## What is tested automatically (no model needed)

* `cargo test --test eval_detection` runs offline language detection over the dataset. Current result: language family correct on **111/111**, exact label on 83/111 (most "misses" are Romanized vs. Mixed, which are both handled the same way). ⚠️ This is in-sample: the lexicon was extended after looking at misses in this set, so expect lower accuracy on new text.
* `npm run eval:check` validates the dataset's structure.

## Live model evaluation (human-reviewed)

```bash
cd src-tauri
cargo run --release --example eval -- --provider ollama --model gemma3:4b
API_KEY=... cargo run --release --example eval -- --provider anthropic --model <model-id> --limit 30
API_KEY=... cargo run --release --example eval -- --provider openrouter --model <model-id>
cargo run --release --example eval -- --provider custom --url http://127.0.0.1:8000/v1 --model <model-id>
```

This writes `eval-reports/<provider>-<model>-<time>.md` (git-ignored). It shows automatic signals (valid JSON rate, `must_keep` preserved, negation preserved, mean latency) plus a **score** column for a bilingual reviewer:

* **2**: meaning faithful (tense, negation, certainty, actor all correct)
* **1**: partly right or awkward but not misleading
* **0**: wrong, invented content, or lost negation

Report the mean human score per category. **Never report string equality with the reference as accuracy.**

## Results

No live model results yet: this machine had no Ollama model installed and no cloud API key while the app was being built. Add a results table here after your first review run.
