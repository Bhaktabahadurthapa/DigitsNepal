import type { Mode } from "./prompt";

export interface WordMeaning { source: string; devanagari: string | null; meaning: string; literal: string | null; note: string | null; uncertain: boolean }
export interface GrammarNote { original: string; corrected: string; explanation: string }
export interface Translation {
  detected_language: "romanized_nepali" | "native_nepali" | "english" | "mixed";
  nepali_script: string | null;
  accurate_english: string;
  styled_english: string | null;
  grammar_notes: GrammarNote[];
  word_meanings: WordMeaning[];
  ambiguities: string[];
  warnings: string[];
}

const str = (v: unknown, max = 4000): string | null => (typeof v === "string" && v.trim() ? v.trim().slice(0, max) : null);
const arr = (v: unknown): unknown[] => (Array.isArray(v) ? v.slice(0, 60) : []);
const LANGS = ["romanized_nepali", "native_nepali", "english", "mixed"] as const;
const norm = (s: string) => s.toLowerCase().normalize("NFC").replace(/\s+/g, " ").trim();

export function extractJson(raw: string): unknown {
  const t = raw.trim().replace(/^```(?:json)?\s*/i, "").replace(/```\s*$/, "");
  try {
    return JSON.parse(t);
  } catch {
    const a = t.indexOf("{");
    const b = t.lastIndexOf("}");
    if (a >= 0 && b > a) {
      try { return JSON.parse(t.slice(a, b + 1)); } catch { /* fall through */ }
    }
    return null;
  }
}

const NE_NEGATION = /\b(chaina|chhaina|chhainan|chainan|hoina|bhayena|bhaena|bhayeena|gardaina|gardina|garena|gareena|sakina|sakdaina|sakena|thiyena|aaudaina|audaina|jaadaina|janna|thaina|natra|nagar\w*|nabhay\w*)\b|छैन|होइन|भएन|सकिन/i;
const EN_NEGATION = /\b(not|no|never|none|nothing|nobody|neither|nor|without|unable|cannot|can't|won't|didn't|doesn't|don't|isn't|wasn't|aren't|weren't|haven't|hasn't|hadn't|couldn't|shouldn't|wouldn't|fail(?:ed|s)?|unclear|unknown|refuse\w*)\b|n't/i;

/** Validate and normalise the model's JSON. Returns null when unusable. */
export function validate(raw: unknown, source: string, mode: Mode): Translation | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const accurate = str(o.accurate_english);
  if (!accurate) return null;
  const warnings: string[] = [];
  const src = norm(source);

  const lang = LANGS.includes(o.detected_language as (typeof LANGS)[number]) ? (o.detected_language as Translation["detected_language"]) : "mixed";

  const meanings: WordMeaning[] = [];
  let dropped = 0;
  for (const m of arr(o.word_meanings)) {
    if (!m || typeof m !== "object") continue;
    const w = m as Record<string, unknown>;
    const s = str(w.source, 200);
    const meaning = str(w.meaning, 400);
    if (!s || !meaning) continue;
    if (!src.includes(norm(s))) { dropped++; continue; }
    meanings.push({ source: s, devanagari: str(w.devanagari, 200), meaning, literal: str(w.literal, 400), note: str(w.note, 400), uncertain: w.uncertain === true });
  }
  if (dropped) warnings.push(`Removed ${dropped} word explanation${dropped === 1 ? "" : "s"} that did not match your text.`);

  const notes: GrammarNote[] = [];
  if (mode === "grammar") {
    for (const n of arr(o.grammar_notes)) {
      if (!n || typeof n !== "object") continue;
      const g = n as Record<string, unknown>;
      const original = str(g.original, 400), corrected = str(g.corrected, 400), explanation = str(g.explanation, 600);
      if (original && corrected && explanation) notes.push({ original, corrected, explanation });
    }
  }

  // Faithfulness checks (warnings only; never rewrite the output).
  const english = [accurate, str(o.styled_english) ?? ""].join(" ");
  if (lang !== "english" && NE_NEGATION.test(source) && !EN_NEGATION.test(accurate)) {
    warnings.push("The source looks negative but the translation may not be. Please check.");
  }
  for (const n of source.match(/\d+(?:[.,:]\d+)*/g) ?? []) {
    if (!english.includes(n)) { warnings.push(`The number ${n} does not appear in the translation.`); break; }
  }

  return {
    detected_language: lang,
    nepali_script: lang === "english" ? null : str(o.nepali_script),
    accurate_english: accurate,
    styled_english: mode === "grammar" ? null : str(o.styled_english),
    grammar_notes: notes,
    word_meanings: meanings,
    ambiguities: arr(o.ambiguities).map((a) => str(a, 400)).filter((a): a is string => !!a).slice(0, 10),
    warnings,
  };
}
