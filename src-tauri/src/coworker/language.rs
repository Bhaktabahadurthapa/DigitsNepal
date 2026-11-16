//! Deterministic Nepali language understanding, offered to the agent as the
//! `dn.understand_text` tool. It runs offline (no model call): language
//! detection plus dictionary glosses from DigitsNepal's lexicon, so the model
//! can ground its reading of Romanized Nepali before planning.

use crate::translation::{detect::detect, wordgloss::gloss};
use serde_json::{json, Value};

pub const MAX_CHARS: usize = 4000;

pub fn understand_text(text: &str) -> Value {
    let text: String = text.chars().take(MAX_CHARS).collect();
    let d = detect(&text);
    let glosses: Vec<Value> = gloss(&text)
        .into_iter()
        .take(60)
        .map(|w| json!({"source": w.source, "devanagari": w.devanagari, "meaning": w.meaning}))
        .collect();
    json!({
        "detected_language": d.language,
        "uncertain": d.uncertain,
        "dictionary_glosses": glosses,
        "guidance": "Glosses are dictionary meanings without sentence context. Reply in the user's language: if they wrote Romanized Nepali, answer in simple English and add a short Romanized Nepali summary."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understands_romanized_nepali_command_offline() {
        let v = understand_text("mero downloads folder ma pdf haru khojera summarize gara");
        assert_eq!(v["detected_language"], "mixed");
        let sources: Vec<&str> = v["dictionary_glosses"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|g| g["source"].as_str())
            .collect();
        assert!(sources.contains(&"mero"), "{sources:?}");
    }

    #[test]
    fn english_has_no_glosses_needed() {
        let v = understand_text("Find the PDF files in my Downloads folder.");
        assert_eq!(v["detected_language"], "english");
    }
}
