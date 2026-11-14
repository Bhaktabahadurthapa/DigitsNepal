// Browser-preview fixtures, used ONLY by `vite dev` outside Tauri, so the UI
// can be developed and screenshotted without the native app. Every result
// is labelled "demo" in the provider field. Never bundled into app behaviour.
import type { Detection, ProviderInfo, Settings, TranslateResponse, WordMeaning } from "../types/translation";

export const DEMO_SOURCE = "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.";

export const demoSettings: Settings = {
  provider: "ollama",
  providers: { ollama: { model: "demo", base_url: "", cloud_consent: false } },
  default_mode: "accurate",
  shortcut: "Control+Alt+T",
  shortcut_enabled: true,
  history_enabled: false,
  clipboard_fallback: false,
  word_meanings: true,
  auto_translate: true,
  saved_keys: [],
  hide_from_capture: false,
  window_opacity: 100,
  paused: false,
  onboarding_done: true,
};

const words: WordMeaning[] = [
  { source: "Maile", devanagari: "मैले", meaning: "I (as the actor in the sentence)", context: "Ergative pronoun: marks the doer of a past action", uncertain: false, origin: "model" },
  { source: "hijo", devanagari: "हिजो", meaning: "yesterday", context: "Time adverb", uncertain: false, origin: "model" },
  { source: "meeting ma", devanagari: "मिटिङमा", meaning: "in / during the meeting", context: "English noun + Nepali postposition 'ma'", uncertain: false, origin: "model" },
  { source: "technical problem", devanagari: "प्राविधिक समस्या", meaning: "technical problem", context: "English term kept as is", uncertain: false, origin: "model" },
  { source: "ko barema", devanagari: "को बारेमा", meaning: "about", literal: "on the subject of", context: "Postpositional phrase", uncertain: false, origin: "model" },
  { source: "kura gareko thiye", devanagari: "कुरा गरेको थिएँ", meaning: "had discussed / discussed", literal: "had done talk", context: "Past perfect; 'thiye' could also be 3rd-person plural, but 'Maile' fixes it as 'I'", uncertain: false, origin: "model" },
  { source: "tara", devanagari: "तर", meaning: "but", context: "Conjunction", uncertain: false, origin: "model" },
  { source: "solution clear bhayena", devanagari: "समाधान स्पष्ट भएन", meaning: "the solution was not clear", literal: "solution did not become clear", context: "Negative past of 'hunu' (to become)", uncertain: false, origin: "model" },
];

export function demoTranslate(text: string): TranslateResponse {
  if (text.trim() !== DEMO_SOURCE) {
    throw { code: "not_desktop", message: "Browser preview only has demo data for the example sentence. Run the desktop app to translate any text.", retryable: false };
  }
  return {
    history_id: null,
    result: {
      detected_language: "mixed",
      original_text: DEMO_SOURCE,
      nepali_script: "मैले हिजो मिटिङमा प्राविधिक समस्याको बारेमा कुरा गरेको थिएँ, तर समाधान स्पष्ट भएन।",
      accurate_english: "I discussed the technical problem during yesterday's meeting, but the solution was not clear.",
      styled_english: "During yesterday's meeting, I discussed the technical issue, but the solution remained unclear.",
      styled_mode: "professional",
      mode: "accurate",
      word_meanings: words,
      ambiguities: [],
      warnings: [],
      provider: "demo",
      model: "browser-preview fixture",
    },
  };
}

export const demoDetection: Detection = { language: "mixed", uncertain: false, nepali_score: 8, english_score: 0, devanagari_ratio: 0 };
export const demoGloss = (text: string): WordMeaning[] => (text.trim() === DEMO_SOURCE ? words.map((w) => ({ ...w, origin: "dictionary" })) : []);

const pi = (id: string, name: string, protocol: ProviderInfo["protocol"], local: boolean, needs_key: boolean, editable_url: boolean, url: string, key_help: string): ProviderInfo => ({
  id, name, protocol, local, needs_key, editable_url, default_base_url: url, base_url: url || null, key_help, key_present: false, cloud: !local,
});
export const demoProviders: ProviderInfo[] = [
  pi("ollama", "Ollama", "ollama", true, false, true, "http://127.0.0.1:11434", "No key needed."),
  pi("lmstudio", "LM Studio", "open_ai", true, false, true, "http://127.0.0.1:1234/v1", "No key needed."),
  pi("gemini", "Google Gemini", "gemini", false, true, false, "https://generativelanguage.googleapis.com/v1beta", "aistudio.google.com → Get API key"),
  pi("anthropic", "Anthropic Claude", "anthropic", false, true, false, "https://api.anthropic.com/v1", "console.anthropic.com → API Keys"),
  pi("openai", "OpenAI", "open_ai", false, true, false, "https://api.openai.com/v1", "platform.openai.com → API keys"),
  pi("openrouter", "OpenRouter", "open_ai", false, true, false, "https://openrouter.ai/api/v1", "openrouter.ai → Keys"),
  pi("custom", "Custom (OpenAI-compatible)", "open_ai", false, false, true, "", "Any /v1/chat/completions server."),
];
