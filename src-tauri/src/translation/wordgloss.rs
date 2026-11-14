//! Offline dictionary glosses for the Word Meaning Explorer.
//!
//! Longest-match over the curated lexicon. These are dictionary meanings,
//! labelled `origin: "dictionary"`, and ambiguous romanizations are marked
//! uncertain. Contextual meaning requires a model.

use super::detect::latin_tokens;
use super::lexicon::{self, PHRASES};
use super::types::WordMeaning;

pub fn gloss(text: &str) -> Vec<WordMeaning> {
    let tokens = latin_tokens(text);
    let mut out = Vec::new();
    let mut i = 0;
    'outer: while i < tokens.len() {
        // Try phrases of up to 4 tokens, longest first.
        for n in (2..=4).rev() {
            if i + n > tokens.len() {
                continue;
            }
            let cand = tokens[i..i + n].join(" ");
            if let Some(p) = PHRASES.iter().find(|p| p.roman.contains(&cand.as_str())) {
                out.push(entry(&original_span(text, &cand), p));
                i += n;
                continue 'outer;
            }
        }
        if let Some(e) = lexicon::lookup_word(&tokens[i]) {
            out.push(entry(&original_span(text, &tokens[i]), e));
        }
        i += 1;
    }
    out
}

fn entry(source: &str, e: &lexicon::Entry) -> WordMeaning {
    WordMeaning {
        source: source.to_string(),
        devanagari: Some(e.devanagari.to_string()),
        literal: None,
        meaning: e.gloss.to_string(),
        context: if e.ambiguous {
            "Dictionary gloss — this spelling has more than one possible meaning.".into()
        } else {
            "Dictionary gloss — not checked against sentence context.".into()
        },
        uncertain: e.ambiguous,
        origin: "dictionary".into(),
    }
}

/// Recover the user's original capitalisation for a lowercase token span.
fn original_span(text: &str, lower: &str) -> String {
    let words: Vec<&str> = lower.split(' ').collect();
    let orig: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphabetic() || c == '\''))
        .filter(|t| !t.is_empty())
        .collect();
    for w in orig.windows(words.len()) {
        if w.iter().zip(&words).all(|(a, b)| a.eq_ignore_ascii_case(b)) {
            return w.join(" ");
        }
    }
    lower.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glosses_spec_sentence_with_phrases() {
        let g =
            gloss("Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.");
        let src: Vec<&str> = g.iter().map(|w| w.source.as_str()).collect();
        assert!(src.contains(&"Maile"));
        assert!(src.contains(&"hijo"));
        assert!(src.contains(&"ko barema"), "{src:?}");
        assert!(src.contains(&"tara"));
        assert!(src.contains(&"clear bhayena"));
        assert!(g.iter().all(|w| w.origin == "dictionary"));
        let ma = g.iter().find(|w| w.source == "ma").unwrap();
        assert!(ma.uncertain, "'ma' is ambiguous");
    }

    #[test]
    fn preserves_original_spelling() {
        let g = gloss("MAILE Hijo");
        assert_eq!(g[0].source, "MAILE");
        assert_eq!(g[1].source, "Hijo");
    }

    #[test]
    fn english_yields_nothing() {
        assert!(gloss("The deployment completed").is_empty());
    }
}
