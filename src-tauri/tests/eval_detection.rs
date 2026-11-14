//! Runs offline language detection over the evaluation dataset.
//! This is deterministic and needs no model.

use digitsnepal_lib::translation::detect::detect;
use digitsnepal_lib::translation::types::DetectedLanguage::{self, *};
use serde_json::Value;

fn family(l: DetectedLanguage) -> &'static str {
    match l {
        RomanizedNepali | NativeNepali | Mixed => "nepali",
        English => "english",
        Unknown => "unknown",
    }
}

#[test]
fn detection_over_eval_dataset() {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/eval-dataset.json"
    ))
    .unwrap();
    let v: Value = serde_json::from_str(&raw).unwrap();
    let cases = v["cases"].as_array().unwrap();
    let (mut exact, mut fam, mut total) = (0, 0, 0);
    let mut misses = Vec::new();
    for c in cases {
        let expected: DetectedLanguage = serde_json::from_value(c["expected_language"].clone()).unwrap();
        let got = detect(c["source"].as_str().unwrap()).language;
        total += 1;
        if got == expected {
            exact += 1;
        }
        if family(got) == family(expected) {
            fam += 1;
        } else {
            misses.push(format!("{}: expected {:?}, got {:?}", c["id"], expected, got));
        }
    }
    let fam_acc = fam as f64 / total as f64;
    println!(
        "detection exact={exact}/{total} family={fam}/{total} ({:.1}%)",
        fam_acc * 100.0
    );
    for m in &misses {
        println!("  miss {m}");
    }
    assert!(fam_acc >= 0.95, "language-family detection below 95%: {misses:?}");
}
