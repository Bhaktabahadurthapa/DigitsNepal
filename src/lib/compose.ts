// Pure logic for Live Compose: decide when a draft is "settled" enough to
// translate without interrupting the user mid-word.

export const PAUSE_MS = 1200;

const SENTENCE_END = /[.!?।॥]["')\]]?\s*$/u;

/** True if the user is in the middle of a word (last char is a letter/digit). */
export function midWord(text: string): boolean {
  return /[\p{L}\p{N}\p{M}]$/u.test(text);
}

export function endsSentence(text: string): boolean {
  return SENTENCE_END.test(text.trimEnd()) && /\s$|[.!?।॥]$/u.test(text);
}

/**
 * Translate when the draft changed and either a sentence just ended or the
 * user paused for PAUSE_MS — but never while a word is half-typed unless
 * the pause is long (2× PAUSE_MS).
 */
export function shouldTranslate(text: string, lastTranslated: string, idleMs: number): boolean {
  const t = text.trim();
  if (!t || t === lastTranslated.trim()) return false;
  if (endsSentence(text)) return true;
  if (midWord(text)) return idleMs >= PAUSE_MS * 2;
  return idleMs >= PAUSE_MS;
}

/** Split into sentences, keeping Devanagari danda as a boundary. */
export function splitSentences(text: string): string[] {
  return text
    .split(/(?<=[.!?।॥])\s+/u)
    .map((s) => s.trim())
    .filter(Boolean);
}
