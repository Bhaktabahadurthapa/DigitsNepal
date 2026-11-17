// Mirrors src-tauri/src/translation/types.rs and commands/mod.rs.

export type TranslationMode = "accurate" | "professional" | "casual" | "academic" | "technical" | "grammar";
export type DetectedLanguage = "romanized_nepali" | "native_nepali" | "english" | "mixed" | "unknown";
export type ProviderId = string;

export interface ProviderConfig {
  model: string;
  base_url: string;
  cloud_consent: boolean;
}

export interface ProviderInfo {
  id: ProviderId;
  name: string;
  protocol: "ollama" | "gemini" | "anthropic" | "open_ai";
  default_base_url: string;
  local: boolean;
  needs_key: boolean;
  editable_url: boolean;
  key_help: string;
  key_present: boolean;
  base_url: string | null;
  cloud: boolean;
}

export const MODES: { id: TranslationMode; label: string; hint: string }[] = [
  { id: "accurate", label: "Accurate", hint: "Preserve the original meaning" },
  { id: "professional", label: "Professional", hint: "Polished workplace English" },
  { id: "casual", label: "Casual", hint: "Natural everyday English" },
  { id: "academic", label: "Academic", hint: "Formal, no invented facts" },
  { id: "technical", label: "Technical", hint: "Keeps engineering terms verbatim" },
  { id: "grammar", label: "Grammar", hint: "Fix English without changing meaning" },
];

export const LANGUAGE_LABEL: Record<DetectedLanguage, string> = {
  romanized_nepali: "Romanized Nepali",
  native_nepali: "Nepali (Devanagari)",
  english: "English",
  mixed: "Mixed Nepali-English",
  unknown: "Unknown",
};

export interface WordMeaning {
  source: string;
  devanagari?: string | null;
  literal?: string | null;
  meaning: string;
  context: string;
  uncertain: boolean;
  origin: "model" | "dictionary" | string;
}

export interface TranslationResult {
  detected_language: DetectedLanguage;
  original_text: string;
  nepali_script: string | null;
  accurate_english: string;
  styled_english: string | null;
  styled_mode: TranslationMode | null;
  mode: TranslationMode;
  word_meanings: WordMeaning[];
  ambiguities: string[];
  warnings: string[];
  provider: string;
  model: string;
}

export interface TranslateResponse {
  result: TranslationResult;
  history_id: string | null;
}

export interface Detection {
  language: DetectedLanguage;
  uncertain: boolean;
  nepali_score: number;
  english_score: number;
  devanagari_ratio: number;
}

export interface AppError {
  code: string;
  message: string;
  retryable: boolean;
}

export interface Settings {
  provider: ProviderId;
  providers: Record<ProviderId, ProviderConfig>;
  default_mode: TranslationMode;
  shortcut: string;
  shortcut_enabled: boolean;
  history_enabled: boolean;
  clipboard_fallback: boolean;
  word_meanings: boolean;
  auto_translate: boolean;
  saved_keys: string[];
  hide_from_capture: boolean;
  window_opacity: number;
  paused: boolean;
  typing_button: boolean;
  onboarding_done: boolean;
}

export interface ModelInfo {
  id: string;
  display_name: string;
  description: string;
}

export interface HistoryEntry {
  id: string;
  created_at: string;
  favorite: boolean;
  mode: TranslationMode;
  original_text: string;
  accurate_english: string;
  styled_english: string | null;
  nepali_script: string | null;
  provider: string;
  model: string;
}

export interface PendingCapture {
  status: "ok" | "permission_required" | "secure_input" | "no_selection" | "unsupported" | "clipboard" | "not_mac_os" | "";
  text: string | null;
  message: string | null;
  can_replace: boolean;
  clipboard_fallback: boolean;
  seq: number;
}
