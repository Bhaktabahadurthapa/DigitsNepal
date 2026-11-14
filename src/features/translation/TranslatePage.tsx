import { useEffect, useState } from "react";
import { Button, CopyButton, ErrorBanner, Notice, Panel, Spinner } from "../../components/ui";
import { ModeSelect } from "../../components/ModeSelect";
import { WordMeaningTable } from "../../components/WordMeaningTable";
import { useTranslate } from "../../hooks/useTranslate";
import { api, isTauri } from "../../providers/api";
import { LANGUAGE_LABEL, MODES, type Detection, type Settings, type TranslationMode, type WordMeaning } from "../../types/translation";

const PROVIDER_NAMES: Record<string, string> = {
  ollama: "Ollama (local)", lmstudio: "LM Studio (local)", gemini: "Gemini (cloud)", anthropic: "Claude (cloud)", openai: "OpenAI (cloud)",
  openrouter: "OpenRouter (cloud)", groq: "Groq (cloud)", mistral: "Mistral (cloud)", deepseek: "DeepSeek (cloud)", together: "Together (cloud)", xai: "xAI (cloud)", custom: "Custom",
};
const LOCAL_PROVIDERS = new Set(["ollama", "lmstudio"]);

const EXAMPLE = "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.";

export function TranslatePage({ settings, onOpenSettings, onExplore, initial = "" }: { settings: Settings | null; onOpenSettings: () => void; onExplore: (text: string, meanings?: WordMeaning[]) => void; initial?: string }) {
  const [text, setText] = useState(initial);
  const [mode, setMode] = useState<TranslationMode>(settings?.default_mode ?? "accurate");
  const [showWords, setShowWords] = useState(settings?.word_meanings ?? true);
  const [isPrivate, setPrivate] = useState(false);
  const [detection, setDetection] = useState<Detection | null>(null);
  const [saved, setSaved] = useState(false);
  const t = useTranslate();

  useEffect(() => {
    if (settings) setMode(settings.default_mode);
  }, [settings?.default_mode]);

  // Local, offline language detection as you type.
  useEffect(() => {
    if ((!isTauri() && !import.meta.env.DEV) || !text.trim()) {
      setDetection(null);
      return;
    }
    const h = setTimeout(() => api.detectLanguage(text).then(setDetection).catch(() => {}), 250);
    return () => clearTimeout(h);
  }, [text]);

  const translate = () => {
    setSaved(false);
    return t.run(text, mode, showWords, isPrivate);
  };

  const r = t.result;
  const styledLabel = r?.styled_mode ? MODES.find((m) => m.id === r.styled_mode)?.label + " English" : null;
  const activeModel = settings?.providers[settings.provider]?.model;
  const providerLabel = settings ? `${PROVIDER_NAMES[settings.provider] ?? settings.provider}${activeModel ? ` · ${activeModel}` : ""}` : "—";
  const providerIsLocal = settings ? LOCAL_PROVIDERS.has(settings.provider) : true;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center gap-2">
        <ModeSelect value={mode} onChange={setMode} />
        <div className="flex-1" />
        <button type="button" onClick={onOpenSettings} className="text-[12px] text-muted hover:text-ink" title="Change provider in Settings">
          Provider: <span className={providerIsLocal ? "text-ok" : "text-crimson"}>{providerLabel}</span>
        </button>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel
          title={
            <span>
              Original
              {detection && (
                <span className="ml-2 font-normal normal-case tracking-normal">
                  · {LANGUAGE_LABEL[detection.language]}
                  {detection.uncertain ? " (uncertain)" : ""}
                </span>
              )}
            </span>
          }
          actions={
            <>
              {!text && (
                <Button variant="ghost" onClick={() => setText(EXAMPLE)}>
                  Try an example
                </Button>
              )}
              {text && (
                <Button variant="ghost" onClick={() => { setText(""); t.reset(); }}>
                  Clear
                </Button>
              )}
            </>
          }
        >
          <textarea
            aria-label="Text to translate"
            className="h-full min-h-40 w-full resize-none bg-transparent p-3 text-[15px] leading-relaxed outline-none"
            placeholder="Type or paste Romanized Nepali, Devanagari, or mixed text…  e.g. Yo error kina aayo?"
            value={text}
            spellCheck={false}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                translate();
              }
            }}
          />
        </Panel>

        <Panel title="English" actions={r && <CopyButton text={r.accurate_english} label="Copy" />}>
          <div className="flex flex-col gap-3 p-3">
            {t.loading && <Spinner />}
            <ErrorBanner error={t.error} onRetry={translate} />
            {!r && !t.loading && !t.error && <p className="text-muted">Press Translate or ⌘↩ to see the English translation here.</p>}
            {r && (
              <>
                <Block label={mode === "grammar" ? "Corrected English" : "Accurate English"} text={r.accurate_english} big />
                {r.styled_english && styledLabel && <Block label={styledLabel} text={r.styled_english} />}
                {r.nepali_script && <Block label="Nepali (Devanagari)" text={r.nepali_script} deva />}
                {r.warnings.map((w) => (
                  <Notice key={w}>{w}</Notice>
                ))}
                {r.ambiguities.length > 0 && (
                  <div>
                    <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-muted">Ambiguities</div>
                    <ul className="list-disc pl-5 text-[12.5px]">
                      {r.ambiguities.map((a) => (
                        <li key={a}>{a}</li>
                      ))}
                    </ul>
                  </div>
                )}
                <div className="text-[11px] text-muted">
                  {LANGUAGE_LABEL[r.detected_language]} · {r.provider} · {r.model}
                </div>
              </>
            )}
          </div>
        </Panel>
      </div>

      {r && showWords && r.word_meanings.length > 0 && (
        <Panel title="Word meanings" className="max-h-64" actions={<Button variant="ghost" onClick={() => onExplore(r.original_text, r.word_meanings)}>Open in Word Explorer</Button>}>
          <WordMeaningTable items={r.word_meanings} />
        </Panel>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button variant="primary" onClick={translate} disabled={!text.trim() || t.loading} aria-keyshortcuts="Meta+Enter">
          Translate <span className="opacity-70">⌘↩</span>
        </Button>
        <label className="flex items-center gap-1.5 text-[12px] text-muted">
          <input type="checkbox" checked={showWords} onChange={(e) => setShowWords(e.target.checked)} className="accent-[var(--accent)]" />
          Show word meanings
        </label>
        {settings?.history_enabled && (
          <label className="flex items-center gap-1.5 text-[12px] text-muted" title="Do not record this translation in history">
            <input type="checkbox" checked={isPrivate} onChange={(e) => setPrivate(e.target.checked)} className="accent-[var(--accent)]" />
            Private (don't save to history)
          </label>
        )}
        <div className="flex-1" />
        {r && (
          <Button
            disabled={saved}
            onClick={async () => {
              try {
                await api.historySave(r);
                setSaved(true);
              } catch {
                /* surfaced elsewhere */
              }
            }}
            title="Save to favorites in local history"
          >
            {saved ? "Saved" : "Save"}
          </Button>
        )}
      </div>
    </div>
  );
}

function Block({ label, text, big, deva }: { label: string; text: string; big?: boolean; deva?: boolean }) {
  return (
    <div className="group rounded-md border border-line bg-surface-2 p-2.5">
      <div className="mb-1 flex items-center justify-between">
        <span className="text-[11px] font-semibold uppercase tracking-wide text-muted">{label}</span>
        <CopyButton text={text} />
      </div>
      <p data-selectable className={`${big ? "text-[15.5px]" : "text-[14px]"} leading-relaxed ${deva ? "deva" : ""}`}>
        {text}
      </p>
    </div>
  );
}
