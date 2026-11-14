//! Shared, serialisable types for the translation engine.
//!
//! These cross the Tauri IPC boundary, so field names are snake_case and
//! mirrored in `src/types/translation.ts`.

use serde::{Deserialize, Serialize};

/// Hard upper bound on input length (characters). Longer text is rejected
/// up-front instead of being silently truncated.
pub const MAX_INPUT_CHARS: usize = 6_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TranslationMode {
    /// Preserve the original meaning as closely as natural English allows.
    #[default]
    Accurate,
    Professional,
    Casual,
    Academic,
    Technical,
    /// Improve existing English without changing its meaning.
    Grammar,
}

impl TranslationMode {
    pub const ALL: [TranslationMode; 6] = [
        TranslationMode::Accurate,
        TranslationMode::Professional,
        TranslationMode::Casual,
        TranslationMode::Academic,
        TranslationMode::Technical,
        TranslationMode::Grammar,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TranslationMode::Accurate => "Accurate Translation",
            TranslationMode::Professional => "Professional English",
            TranslationMode::Casual => "Casual English",
            TranslationMode::Academic => "Academic English",
            TranslationMode::Technical => "Technical English",
            TranslationMode::Grammar => "Grammar Correction",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            TranslationMode::Accurate => "accurate",
            TranslationMode::Professional => "professional",
            TranslationMode::Casual => "casual",
            TranslationMode::Academic => "academic",
            TranslationMode::Technical => "technical",
            TranslationMode::Grammar => "grammar",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectedLanguage {
    RomanizedNepali,
    NativeNepali,
    English,
    Mixed,
    Unknown,
}

impl DetectedLanguage {
    pub fn from_model_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
            "romanized_nepali" | "roman_nepali" | "romanised_nepali" => Some(Self::RomanizedNepali),
            "native_nepali" | "nepali" | "devanagari" | "devanagari_nepali" => Some(Self::NativeNepali),
            "english" => Some(Self::English),
            "mixed" | "mixed_nepali_english" | "code_mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordMeaning {
    /// Exactly as it appears in the user's text.
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devanagari: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal: Option<String>,
    /// Meaning in this sentence.
    pub meaning: String,
    #[serde(default)]
    pub context: String,
    /// True when the interpretation is ambiguous or could not be verified
    /// against the input text.
    #[serde(default)]
    pub uncertain: bool,
    /// "model" or "dictionary" — where this gloss came from.
    #[serde(default = "default_origin")]
    pub origin: String,
}

fn default_origin() -> String {
    "model".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    #[serde(default)]
    pub mode: TranslationMode,
    #[serde(default = "yes")]
    pub include_word_meanings: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationResult {
    pub detected_language: DetectedLanguage,
    /// Always the exact text the user submitted — never the model's echo.
    pub original_text: String,
    pub nepali_script: Option<String>,
    /// Faithful translation (or minimally corrected English in Grammar mode).
    pub accurate_english: String,
    /// Mode-styled rendering, kept separate from the faithful version.
    /// Accurate mode carries a Professional rendering here for side-by-side use.
    pub styled_english: Option<String>,
    pub styled_mode: Option<TranslationMode>,
    pub mode: TranslationMode,
    pub word_meanings: Vec<WordMeaning>,
    pub ambiguities: Vec<String>,
    /// Deterministic post-checks raised by DigitsNepal (not by the model).
    pub warnings: Vec<String>,
    pub provider: String,
    pub model: String,
}
