import type { WordMeaning } from "../types/translation";

export type Segment = { text: string; index: number | null };

/**
 * Map word-meaning entries back onto the original text, in order, so each
 * Nepali word/phrase can be clicked. Entries that cannot be found are left
 * out of the inline view (they still appear in the table, flagged).
 */
export function alignMeanings(text: string, meanings: WordMeaning[]): Segment[] {
  const lower = text.toLowerCase();
  const spans: { start: number; end: number; index: number }[] = [];
  let cursor = 0;
  meanings.forEach((m, index) => {
    const needle = m.source.trim().toLowerCase();
    if (!needle) return;
    let at = findWord(lower, needle, cursor);
    if (at < 0) at = findWord(lower, needle, 0); // out-of-order entry
    if (at < 0) return;
    const end = at + needle.length;
    if (spans.some((s) => at < s.end && end > s.start)) return; // overlap
    spans.push({ start: at, end, index });
    if (at >= cursor) cursor = end;
  });
  spans.sort((a, b) => a.start - b.start);
  const out: Segment[] = [];
  let pos = 0;
  for (const s of spans) {
    if (s.start > pos) out.push({ text: text.slice(pos, s.start), index: null });
    out.push({ text: text.slice(s.start, s.end), index: s.index });
    pos = s.end;
  }
  if (pos < text.length) out.push({ text: text.slice(pos), index: null });
  return out;
}

const isWordChar = (c: string | undefined) => !!c && /[\p{L}\p{N}\p{M}]/u.test(c);

function findWord(hay: string, needle: string, from: number): number {
  let i = hay.indexOf(needle, from);
  while (i >= 0) {
    if (!isWordChar(hay[i - 1]) && !isWordChar(hay[i + needle.length])) return i;
    i = hay.indexOf(needle, i + 1);
  }
  return -1;
}
