import { useEffect, useMemo, useState } from "react";
import { Button, ErrorBanner, Notice, Panel, Spinner } from "../../components/ui";
import { WordMeaningTable } from "../../components/WordMeaningTable";
import { useTranslate } from "../../hooks/useTranslate";
import { alignMeanings } from "../../lib/align";
import { api, isTauri } from "../../providers/api";
import type { WordMeaning } from "../../types/translation";

export function WordExplorer({ initialText, initialMeanings }: { initialText: string; initialMeanings?: WordMeaning[] }) {
  const [text, setText] = useState(initialText);
  const [dictionary, setDictionary] = useState<WordMeaning[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const t = useTranslate();

  useEffect(() => setText(initialText), [initialText]);

  // Offline dictionary glosses — instant, no model needed.
  useEffect(() => {
    if ((!isTauri() && !import.meta.env.DEV) || !text.trim()) {
      setDictionary([]);
      return;
    }
    const h = setTimeout(() => api.dictionaryGloss(text).then(setDictionary).catch(() => setDictionary([])), 200);
    return () => clearTimeout(h);
  }, [text]);

  const fromModel = t.result && t.result.original_text === text.trim() ? t.result.word_meanings : null;
  const handedOver = initialMeanings?.length && text === initialText ? initialMeanings : null;
  const contextual = fromModel ?? handedOver;
  const items = contextual ?? dictionary;
  const segments = useMemo(() => alignMeanings(text, items), [text, items]);
  const current = selected !== null ? items[selected] : null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <div className="flex items-center gap-2">
        <input
          aria-label="Sentence to explore"
          className="h-8 flex-1 rounded-md border border-line bg-surface px-3 text-[14px] outline-none"
          placeholder="Paste a Nepali sentence to inspect each word…"
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setSelected(null);
          }}
          onKeyDown={(e) => e.key === "Enter" && text.trim() && t.run(text, "accurate", true)}
        />
        <Button variant="primary" disabled={!text.trim() || t.loading} onClick={() => t.run(text, "accurate", true)}>
          Explain in context
        </Button>
      </div>

      <ErrorBanner error={t.error} onRetry={() => t.run(text, "accurate", true)} />
      {t.loading && <Spinner label="Explaining words in context…" />}
      {!contextual && items.length > 0 && (
        <Notice tone="info">Showing offline dictionary glosses. Use “Explain in context” for sentence-level meanings from your AI provider.</Notice>
      )}

      {text.trim() && (
        <div className="rounded-lg border border-line bg-surface p-4 text-[17px] leading-loose" aria-label="Clickable words">
          {segments.map((s, i) =>
            s.index === null ? (
              <span key={i}>{s.text}</span>
            ) : (
              <button
                key={i}
                type="button"
                onClick={() => setSelected(s.index)}
                aria-pressed={selected === s.index}
                className={`rounded px-0.5 underline decoration-dotted underline-offset-4 transition-colors ${
                  selected === s.index ? "bg-accent text-white dark:text-[#0d1630]" : items[s.index]?.uncertain ? "decoration-crimson hover:bg-warn-bg" : "decoration-accent hover:bg-accent-soft"
                }`}
              >
                {s.text}
              </button>
            ),
          )}
        </div>
      )}

      {current && (
        <div className="rounded-lg border border-line bg-surface p-4" aria-live="polite">
          <div className="flex items-baseline gap-3">
            <span className="text-[18px] font-semibold">{current.source}</span>
            {current.devanagari && <span className="deva text-[18px] text-muted">{current.devanagari}</span>}
            {current.uncertain && <span className="rounded bg-warn-bg px-1.5 py-0.5 text-[11px] font-medium text-warn-text">Uncertain</span>}
          </div>
          <p className="mt-1 text-[14px]">{current.meaning}</p>
          {current.literal && current.literal !== current.meaning && <p className="text-[12.5px] text-muted">Literal meaning: {current.literal}</p>}
          {current.context && <p className="mt-1 text-[12.5px] text-muted">{current.context}</p>}
        </div>
      )}

      <Panel title={contextual ? "Meanings in this sentence" : "Dictionary"} className="min-h-0 flex-1">
        <WordMeaningTable items={items} highlight={selected} onHover={() => {}} />
      </Panel>
    </div>
  );
}
