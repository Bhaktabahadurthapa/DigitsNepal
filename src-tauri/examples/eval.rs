//! Live translation evaluation against the dataset.
//!
//!   cargo run --example eval -- --provider ollama --model gemma3 [--limit 20] [--category technical]
//!   API_KEY=... cargo run --example eval -- --provider gemini --model <model-id>
//!   API_KEY=... cargo run --example eval -- --provider openrouter --model <model-id>
//!   cargo run --example eval -- --provider custom --url http://127.0.0.1:8000/v1 --model <model-id>
//!
//! --provider is any preset id from translation::registry (ollama, lmstudio,
//! gemini, anthropic, openai, openrouter, groq, mistral, deepseek, together,
//! xai, custom). Cloud providers are used only because you ran this command.
//!
//! Writes eval-reports/<provider>-<model>-<time>.{jsonl,md}. Automatic checks
//! (must_keep terms, negation, warnings, latency) are only signals; semantic
//! accuracy must be scored by a human reviewer in the generated Markdown.

use digitsnepal_lib::translation::engine::{translate, RetryPolicy};
use digitsnepal_lib::translation::provider::Provider;
use digitsnepal_lib::translation::registry::{self, ProviderConfig};
use digitsnepal_lib::translation::types::{TranslateRequest, TranslationMode};
use serde_json::{json, Value};
use std::io::Write;
use std::time::Instant;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn english_negated(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("n't")
        || l.split(|c: char| !c.is_alphabetic()).any(|w| {
            matches!(
                w,
                "not"
                    | "no"
                    | "never"
                    | "nobody"
                    | "nothing"
                    | "none"
                    | "unable"
                    | "cannot"
                    | "failed"
                    | "fail"
                    | "unclear"
                    | "without"
            )
        })
}

#[tokio::main]
async fn main() {
    let provider_name = arg("--provider").unwrap_or_else(|| "ollama".into());
    let model = arg("--model").expect("--model is required");
    let limit: usize = arg("--limit").and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let category = arg("--category");
    let cfg = ProviderConfig {
        model: model.clone(),
        base_url: arg("--url").unwrap_or_default(),
        cloud_consent: true,
    };
    let provider: Box<dyn Provider> = registry::build(&provider_name, &cfg, std::env::var("API_KEY").ok())
        .unwrap_or_else(|e| panic!("cannot build provider: {e:?}"));

    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/eval-dataset.json"
    ))
    .unwrap();
    let data: Value = serde_json::from_str(&raw).unwrap();
    let cases: Vec<&Value> = data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| category.as_deref().is_none_or(|cat| c["category"] == cat))
        .take(limit)
        .collect();

    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../eval-reports");
    std::fs::create_dir_all(dir).unwrap();
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let base = format!("{dir}/{provider_name}-{}-{stamp}", model.replace([':', '/'], "_"));
    let mut jsonl = std::fs::File::create(format!("{base}.jsonl")).unwrap();
    let mut md = String::from(&*format!(
        "# DigitsNepal evaluation — {provider_name} / {model}\n\nScore each row 0–2 for meaning (0 wrong, 1 partly, 2 faithful).\n\n| id | source | reference | output | auto checks | score |\n|---|---|---|---|---|---|\n"
    ));
    let (mut ok, mut failed, mut keep_ok, mut keep_n, mut neg_ok, mut neg_n, mut ms_total) = (0, 0, 0, 0, 0, 0, 0u128);

    for c in &cases {
        let mode = TranslationMode::from_id(c["mode"].as_str().unwrap()).unwrap_or_default();
        let req = TranslateRequest {
            text: c["source"].as_str().unwrap().into(),
            mode,
            include_word_meanings: false,
        };
        let t = Instant::now();
        let res = translate(provider.as_ref(), &req, &RetryPolicy::default()).await;
        let ms = t.elapsed().as_millis();
        ms_total += ms;
        let (output, checks) = match &res {
            Ok(r) => {
                ok += 1;
                let mut checks = vec![];
                for k in c["must_keep"].as_array().unwrap() {
                    keep_n += 1;
                    if r.accurate_english.contains(k.as_str().unwrap()) {
                        keep_ok += 1;
                    } else {
                        checks.push(format!("missing '{}'", k.as_str().unwrap()));
                    }
                }
                if c["must_negate"] == true {
                    neg_n += 1;
                    if english_negated(&r.accurate_english) {
                        neg_ok += 1;
                    } else {
                        checks.push("negation lost?".into());
                    }
                }
                checks.extend(r.warnings.iter().map(|w| format!("warn: {w}")));
                (r.accurate_english.clone(), checks)
            }
            Err(e) => {
                failed += 1;
                (String::new(), vec![format!("ERROR {}", e.payload().code)])
            }
        };
        writeln!(jsonl, "{}", json!({"id": c["id"], "source": c["source"], "reference": c["reference"], "output": output, "checks": checks, "latency_ms": ms})).unwrap();
        let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | |\n",
            c["id"].as_str().unwrap(),
            esc(c["source"].as_str().unwrap()),
            esc(c["reference"].as_str().unwrap()),
            esc(&output),
            esc(&checks.join("; "))
        ));
        eprintln!(
            "{} {} ms {}",
            c["id"].as_str().unwrap(),
            ms,
            if res.is_ok() { "ok" } else { "ERR" }
        );
    }
    let n = cases.len().max(1);
    let summary = format!(
        "\n## Automatic summary\n\n- cases: {}\n- valid responses: {ok}\n- failures: {failed}\n- must_keep terms preserved: {keep_ok}/{keep_n}\n- negations preserved: {neg_ok}/{neg_n}\n- mean latency: {} ms\n\nSemantic accuracy: fill in the score column and average it. Do not report string-equality as accuracy.\n",
        cases.len(),
        ms_total / n as u128
    );
    md.push_str(&summary);
    std::fs::write(format!("{base}.md"), md).unwrap();
    println!("{summary}\nReport: {base}.md");
}
