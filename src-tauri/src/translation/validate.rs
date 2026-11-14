//! Validation of untrusted model output.
//!
//! Nothing from a model reaches the UI without passing through here. We:
//! * extract a single JSON object (tolerating code fences / leading prose),
//! * require a non-empty `accurate_english`,
//! * always use the user's own text as `original_text`,
//! * drop malformed optional fields instead of failing the whole response,
//! * flag word-meaning entries whose `source` does not occur in the input,
//! * run deterministic faithfulness checks (negation, numbers), and
//! * ignore any model "confidence" values — they are not calibrated.

use super::detect::{detect, is_devanagari, latin_tokens};
use super::types::*;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ValidationError {
    #[error("the model reply did not contain a JSON object")]
    NoJson,
    #[error("the model reply was not valid JSON: {0}")]
    BadJson(String),
    #[error("the model reply is missing the English translation")]
    MissingTranslation,
}

const MAX_WORD_MEANINGS: usize = 80;
const MAX_AMBIGUITIES: usize = 20;
const MAX_FIELD_CHARS: usize = 20_000;

/// Find the outermost JSON object in `raw`.
pub fn extract_json_object(raw: &str) -> Result<&str, ValidationError> {
    let start = raw.find('{').ok_or(ValidationError::NoJson)?;
    let end = raw.rfind('}').ok_or(ValidationError::NoJson)?;
    if end <= start {
        return Err(ValidationError::NoJson);
    }
    Ok(&raw[start..=end])
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(MAX_FIELD_CHARS).collect())
}

fn normalise(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || is_devanagari(c) {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct Context<'a> {
    pub input: &'a str,
    pub mode: TranslationMode,
    pub provider: &'a str,
    pub model: &'a str,
}

pub fn validate(raw: &str, ctx: &Context) -> Result<TranslationResult, ValidationError> {
    let json = extract_json_object(raw)?;
    let v: Value = serde_json::from_str(json).map_err(|e| ValidationError::BadJson(e.to_string()))?;
    if !v.is_object() {
        return Err(ValidationError::NoJson);
    }

    let accurate = str_field(&v, "accurate_english")
        // Some models echo the older schema name.
        .or_else(|| str_field(&v, "english"))
        .ok_or(ValidationError::MissingTranslation)?;

    let local = detect(ctx.input);
    let detected_language = v
        .get("detected_language")
        .and_then(Value::as_str)
        .and_then(DetectedLanguage::from_model_str)
        .unwrap_or(local.language);

    let nepali_script = str_field(&v, "nepali_script").filter(|s| s.chars().any(is_devanagari));

    let styled_english = if ctx.mode == TranslationMode::Grammar {
        None
    } else {
        str_field(&v, "styled_english").or_else(|| str_field(&v, "professional_english"))
    };
    let styled_mode = styled_english.as_ref().map(|_| match ctx.mode {
        TranslationMode::Accurate => TranslationMode::Professional,
        m => m,
    });

    let mut warnings = Vec::new();
    let input_norm = format!(" {} ", normalise(ctx.input));

    let mut word_meanings = Vec::new();
    if let Some(arr) = v.get("word_meanings").and_then(Value::as_array) {
        for item in arr.iter().take(MAX_WORD_MEANINGS) {
            let (Some(source), Some(meaning)) = (str_field(item, "source"), str_field(item, "meaning")) else {
                continue;
            };
            let found = input_norm.contains(&format!(" {} ", normalise(&source)));
            word_meanings.push(WordMeaning {
                devanagari: str_field(item, "devanagari").filter(|s| s.chars().any(is_devanagari)),
                literal: str_field(item, "literal"),
                context: str_field(item, "context").unwrap_or_default(),
                uncertain: item.get("uncertain").and_then(Value::as_bool).unwrap_or(false) || !found,
                origin: "model".into(),
                source,
                meaning,
            });
        }
        let missing = word_meanings
            .iter()
            .filter(|w| !input_norm.contains(&format!(" {} ", normalise(&w.source))))
            .count();
        if missing > 0 {
            warnings.push(format!(
                "{missing} word-meaning entr{} did not match the original text and {} marked uncertain.",
                if missing == 1 { "y" } else { "ies" },
                if missing == 1 { "is" } else { "are" }
            ));
        }
    }

    let ambiguities = v
        .get("ambiguities")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from))
                .take(MAX_AMBIGUITIES)
                .collect()
        })
        .unwrap_or_default();

    warnings.extend(faithfulness_warnings(ctx.input, &accurate));

    Ok(TranslationResult {
        detected_language,
        original_text: ctx.input.to_string(),
        nepali_script,
        accurate_english: accurate,
        styled_english,
        styled_mode,
        mode: ctx.mode,
        word_meanings,
        ambiguities,
        warnings,
        provider: ctx.provider.to_string(),
        model: ctx.model.to_string(),
    })
}

/// Romanized / Devanagari Nepali negation markers.
const NEPALI_NEGATION: &[&str] = &[
    "chaina",
    "chhaina",
    "chainan",
    "chhainan",
    "hoina",
    "hoena",
    "bhayena",
    "bhaena",
    "thiyena",
    "thiena",
    "sakina",
    "sakdina",
    "bujhina",
    "milena",
    "chalena",
    "gardina",
    "hudaina",
    "hundaina",
    "nagarnu",
    "nagara",
    "nagarnus",
    "najanu",
    "naaunu",
    "parena",
    "aayena",
    "ayena",
    "diyena",
    "garena",
    "gayena",
    "khayena",
    "pathayena",
    "dekhina",
    "dekhena",
    "bhetina",
];
const DEVANAGARI_NEGATION: &[&str] = &[
    "छैन",
    "होइन",
    "भएन",
    "थिएन",
    "सकिनँ",
    "सक्दिन",
    "हुँदैन",
    "मिलेन",
    "चलेन",
    "गरेन",
    "आएन",
    "दिएन",
    "नगर",
    "न ",
];
const ENGLISH_NEGATION: &[&str] = &[
    "not", "no", "never", "neither", "nor", "none", "nothing", "nobody", "unable", "cannot", "without", "fail",
    "failed", "fails", "unclear", "lack", "lacks", "lacked", "nowhere",
];

fn has_english_negation(s: &str) -> bool {
    let lower = s.to_lowercase();
    if lower.contains("n't") || lower.contains("n’t") {
        return true;
    }
    latin_tokens(&lower)
        .iter()
        .any(|t| ENGLISH_NEGATION.contains(&t.as_str()))
}

fn has_nepali_negation(s: &str) -> bool {
    let toks = latin_tokens(s);
    toks.iter().any(|t| {
        NEPALI_NEGATION.contains(&t.as_str())
            || (t.ends_with("ena") && t.len() > 5)
            || (t.ends_with("enan") && t.len() > 6)
    }) || DEVANAGARI_NEGATION.iter().any(|m| s.contains(m))
}

fn digits(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.sort();
    out
}

/// Deterministic, conservative checks. They raise warnings; they never
/// change the translation.
pub fn faithfulness_warnings(source: &str, english: &str) -> Vec<String> {
    let mut w = Vec::new();
    if has_nepali_negation(source) && !has_english_negation(english) {
        w.push("The source contains a negation, but the English does not appear to. Please check the meaning.".into());
    }
    let (sd, ed) = (digits(source), digits(english));
    let missing: Vec<_> = sd.iter().filter(|d| !ed.contains(d)).cloned().collect();
    if !missing.is_empty() {
        w.push(format!(
            "Number(s) {} from the source do not appear in the English.",
            missing.join(", ")
        ));
    }
    let src_len = source.chars().count().max(1);
    let out_len = english.chars().count();
    if src_len > 40 && (out_len < src_len / 4 || out_len > src_len * 4) {
        w.push(
            "The translation length differs a lot from the source; something may have been dropped or added.".into(),
        );
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &str =
        "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.";

    fn ctx(mode: TranslationMode) -> Context<'static> {
        Context {
            input: INPUT,
            mode,
            provider: "test",
            model: "m",
        }
    }

    fn good() -> String {
        serde_json::json!({
            "detected_language": "romanized_nepali",
            "original_text": "SOMETHING ELSE",
            "nepali_script": "मैले हिजो मिटिङमा प्राविधिक समस्याको बारेमा कुरा गरेको थिएँ, तर समाधान स्पष्ट भएन।",
            "accurate_english": "I discussed the technical problem during yesterday's meeting, but the solution was not clear.",
            "styled_english": "During yesterday's meeting, I discussed the technical issue, but the solution remained unclear.",
            "word_meanings": [
                {"source": "Maile", "meaning": "I (as the actor)", "context": "ergative", "uncertain": false, "confidence": 0.99},
                {"source": "ko barema", "meaning": "about", "context": ""},
                {"source": "invented word", "meaning": "x", "context": ""},
                {"source": "", "meaning": "dropped"},
                {"meaning": "no source"}
            ],
            "ambiguities": ["'thiye' could be first person or third person plural", 7, ""]
        })
        .to_string()
    }

    #[test]
    fn accepts_valid_response_and_overrides_original() {
        let r = validate(&good(), &ctx(TranslationMode::Accurate)).unwrap();
        assert_eq!(r.original_text, INPUT);
        assert_eq!(r.detected_language, DetectedLanguage::RomanizedNepali);
        assert_eq!(r.styled_mode, Some(TranslationMode::Professional));
        assert_eq!(r.word_meanings.len(), 3);
        assert!(!r.word_meanings[0].uncertain);
        assert!(!r.word_meanings[1].uncertain, "multiword phrase should match");
        assert!(r.word_meanings[2].uncertain, "source not in input must be flagged");
        assert_eq!(r.ambiguities.len(), 1);
        assert!(r.warnings.iter().any(|w| w.contains("did not match")));
        assert!(!r.warnings.iter().any(|w| w.contains("negation")));
    }

    #[test]
    fn tolerates_code_fences_and_prose() {
        let raw = format!("Sure! Here you go:\n```json\n{}\n```", good());
        assert!(validate(&raw, &ctx(TranslationMode::Accurate)).is_ok());
    }

    #[test]
    fn rejects_non_json() {
        assert_eq!(
            validate("I cannot help", &ctx(TranslationMode::Accurate)).unwrap_err(),
            ValidationError::NoJson
        );
        assert!(matches!(
            validate("{not json}", &ctx(TranslationMode::Accurate)).unwrap_err(),
            ValidationError::BadJson(_)
        ));
    }

    #[test]
    fn rejects_missing_or_empty_translation() {
        let raw = r#"{"accurate_english": "   ", "word_meanings": []}"#;
        assert_eq!(
            validate(raw, &ctx(TranslationMode::Accurate)).unwrap_err(),
            ValidationError::MissingTranslation
        );
        assert_eq!(
            validate("{}", &ctx(TranslationMode::Accurate)).unwrap_err(),
            ValidationError::MissingTranslation
        );
    }

    #[test]
    fn wrong_types_are_dropped_not_fatal() {
        let raw = r#"{"accurate_english": "ok", "nepali_script": "not devanagari", "word_meanings": "nope", "ambiguities": {"a":1}, "detected_language": 5}"#;
        let r = validate(raw, &ctx(TranslationMode::Accurate)).unwrap();
        assert_eq!(r.nepali_script, None);
        assert!(r.word_meanings.is_empty());
        assert!(r.ambiguities.is_empty());
        // Falls back to local detection.
        assert!(matches!(
            r.detected_language,
            DetectedLanguage::RomanizedNepali | DetectedLanguage::Mixed
        ));
    }

    #[test]
    fn grammar_mode_never_returns_styled() {
        let r = validate(&good(), &ctx(TranslationMode::Grammar)).unwrap();
        assert_eq!(r.styled_english, None);
        assert_eq!(r.styled_mode, None);
    }

    #[test]
    fn negation_loss_is_flagged() {
        let raw = r#"{"accurate_english": "I discussed the technical problem in yesterday's meeting and the solution was clear."}"#;
        let r = validate(raw, &ctx(TranslationMode::Accurate)).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("negation")), "{:?}", r.warnings);
    }

    #[test]
    fn negation_detection_cases() {
        assert!(has_nepali_negation("pods ready chainan"));
        assert!(has_nepali_negation("समाधान स्पष्ट भएन"));
        assert!(!has_nepali_negation("kaam sakiyo"));
        assert!(has_english_negation("The pods aren't ready"));
        assert!(has_english_negation("It remained unclear"));
        assert!(!has_english_negation("It is done"));
    }

    #[test]
    fn missing_numbers_flagged() {
        let w = faithfulness_warnings("3 ota pods restart bhayo", "The pods restarted.");
        assert!(w.iter().any(|x| x.contains("Number")));
        assert!(faithfulness_warnings("3 ota pods", "3 pods")
            .iter()
            .all(|x| !x.contains("Number")));
    }
}
