//! Offline language detection for Nepali / English / mixed text.
//!
//! Romanized Nepali uses Latin script, so script detection alone is not
//! enough. We combine:
//! * Devanagari character ratio,
//! * hits against a curated Romanized-Nepali lexicon (weighted),
//! * productive Nepali suffixes,
//! * English function words, and
//! * unknown Latin content words (often English technical terms in
//!   code-mixed sentences such as "Kubernetes cluster deploy gare").

use super::lexicon::{self, PHRASES, SUFFIXES};
use super::types::DetectedLanguage;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Detection {
    pub language: DetectedLanguage,
    /// True when the signal is weak (very short text, close scores).
    pub uncertain: bool,
    pub nepali_score: f32,
    pub english_score: f32,
    pub devanagari_ratio: f32,
}

pub fn is_devanagari(c: char) -> bool {
    ('\u{0900}'..='\u{097F}').contains(&c) || ('\u{A8E0}'..='\u{A8FF}').contains(&c)
}

/// Split into lowercase Latin word tokens (apostrophes kept for "didn't").
pub fn latin_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_ascii_alphabetic() || c == '\''))
        .filter(|t| !t.is_empty() && t.chars().any(|c| c.is_ascii_alphabetic()))
        .map(|t| t.trim_matches('\'').to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

pub fn detect(text: &str) -> Detection {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return Detection {
            language: DetectedLanguage::Unknown,
            uncertain: true,
            nepali_score: 0.0,
            english_score: 0.0,
            devanagari_ratio: 0.0,
        };
    }
    let deva = letters.iter().filter(|c| is_devanagari(**c)).count();
    let latin = letters.iter().filter(|c| c.is_ascii_alphabetic()).count();
    let deva_ratio = deva as f32 / letters.len() as f32;

    let tokens = latin_tokens(text);
    let lowered = format!(" {} ", tokens.join(" "));

    let mut nepali = 0.0f32;
    let mut english = 0.0f32;
    let mut unknown = 0usize;

    // Phrases first (strong signal).
    for p in PHRASES {
        for r in p.roman {
            if lowered.contains(&format!(" {r} ")) {
                nepali += 1.0;
            }
        }
    }
    for t in &tokens {
        if let Some(e) = lexicon::lookup_word(t) {
            nepali += e.weight;
        } else if lexicon::is_english_marker(t) {
            english += 1.0;
        } else if SUFFIXES.iter().any(|s| t.len() >= s.len() + 3 && t.ends_with(s)) {
            nepali += 0.3;
        } else {
            unknown += 1;
        }
    }

    let latin_word_count = tokens.len();
    let short = latin_word_count + (deva > 0) as usize * 3 < 3;

    // Devanagari-dominant text.
    if deva_ratio >= 0.5 {
        let has_latin_words = latin >= 3 && latin_word_count >= 1;
        return Detection {
            language: if has_latin_words && deva_ratio < 0.85 {
                DetectedLanguage::Mixed
            } else {
                DetectedLanguage::NativeNepali
            },
            uncertain: false,
            nepali_score: nepali + deva as f32,
            english_score: english,
            devanagari_ratio: deva_ratio,
        };
    }
    if deva > 0 {
        // Some Devanagari inside mostly Latin text.
        return Detection {
            language: DetectedLanguage::Mixed,
            uncertain: false,
            nepali_score: nepali + deva as f32,
            english_score: english,
            devanagari_ratio: deva_ratio,
        };
    }

    let unknown_ratio = if latin_word_count == 0 {
        0.0
    } else {
        unknown as f32 / latin_word_count as f32
    };

    let language = if nepali < 0.6 && english == 0.0 {
        if unknown > 0 {
            // Only content words we don't know — most likely English.
            DetectedLanguage::English
        } else {
            DetectedLanguage::Unknown
        }
    } else if nepali < 0.6 {
        DetectedLanguage::English
    } else if english >= 1.0 && english >= nepali * 0.75 {
        // Lots of English grammar words and some Nepali: English-dominant
        // unless Nepali still clearly carries the grammar.
        if nepali >= 1.5 {
            DetectedLanguage::Mixed
        } else {
            DetectedLanguage::English
        }
    } else if english >= 1.0 || unknown_ratio > 0.34 {
        DetectedLanguage::Mixed
    } else {
        DetectedLanguage::RomanizedNepali
    };

    let margin = (nepali - english).abs();
    Detection {
        language,
        uncertain: short || margin < 1.0 || language == DetectedLanguage::Unknown,
        nepali_score: nepali,
        english_score: english,
        devanagari_ratio: deva_ratio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DetectedLanguage::*;

    fn lang(s: &str) -> DetectedLanguage {
        detect(s).language
    }

    #[test]
    fn romanized_nepali_sentences() {
        assert!(matches!(
            lang("Maile aja project ko update pathauna sakina."),
            RomanizedNepali | Mixed
        ));
        assert_eq!(lang("Yo error kina aayo?"), RomanizedNepali);
        assert_eq!(lang("Malai thaha chaina, bholi feri herchu."), RomanizedNepali);
        assert_eq!(lang("Timi kaha gayeko?"), RomanizedNepali);
    }

    #[test]
    fn spec_acceptance_sentence_is_nepali_family() {
        let d =
            detect("Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.");
        assert!(matches!(d.language, RomanizedNepali | Mixed), "{d:?}");
        assert!(d.nepali_score > 5.0);
    }

    #[test]
    fn code_mixed_technical() {
        assert_eq!(lang("Maile Kubernetes cluster deploy gare."), Mixed);
        assert_eq!(lang("Deployment complete bhayo tara pods ready chainan."), Mixed);
    }

    #[test]
    fn english() {
        assert_eq!(lang("I deployed the Kubernetes cluster."), English);
        assert_eq!(lang("Why did this error occur?"), English);
        assert_eq!(lang("The deployment completed, but the pods are not ready."), English);
    }

    #[test]
    fn devanagari() {
        assert_eq!(
            lang("मैले हिजो मिटिङमा प्राविधिक समस्याको बारेमा कुरा गरेको थिएँ।"),
            NativeNepali
        );
        assert_eq!(lang("मेरो Kubernetes cluster चलेन"), Mixed);
    }

    #[test]
    fn empty_and_symbols() {
        assert_eq!(lang(""), Unknown);
        assert_eq!(lang("123 !!! ..."), Unknown);
        assert!(detect("").uncertain);
    }

    #[test]
    fn short_text_is_uncertain() {
        assert!(detect("ho").uncertain);
    }

    #[test]
    fn english_word_camera_not_nepali() {
        // "camera" ends in "era" but is English.
        assert_eq!(lang("The camera is broken."), English);
    }
}
