export type Mode = "accurate" | "professional" | "grammar";
export type Appearance = "system" | "light" | "dark";

export interface WordMeaning { source: string; devanagari: string | null; meaning: string; literal: string | null; note: string | null; uncertain: boolean }
export interface GrammarNote { original: string; corrected: string; explanation: string }
export interface Quota { used: number; limit: number; resetAt: number }

export interface Translation {
  detected_language: "romanized_nepali" | "native_nepali" | "english" | "mixed";
  nepali_script: string | null;
  accurate_english: string;
  styled_english: string | null;
  grammar_notes: GrammarNote[];
  word_meanings: WordMeaning[];
  ambiguities: string[];
  warnings: string[];
  mode: Mode;
  model: string;
  quota: Quota;
}

export interface HistoryItem { id: string; at: number; text: string; mode: Mode; english: string; nepali: string | null }
