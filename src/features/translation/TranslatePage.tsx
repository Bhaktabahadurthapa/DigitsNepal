import { useEffect, useRef, useState } from "react";
import { Button, CopyButton, ErrorBanner, Notice, Panel, Spinner } from "../../components/ui";
import { ModeSelect } from "../../components/ModeSelect";
import { WordMeaningTable } from "../../components/WordMeaningTable";
import { OverlayControls } from "../../components/OverlayControls";
import { useTranslate } from "../../hooks/useTranslate";
import { api, isTauri } from "../../providers/api";
import { nextStep } from "../../lib/providerReady";
import { shouldTranslate } from "../../lib/compose";
import { LANGUAGE_LABEL, MODES, type Detection, type ProviderInfo, type Settings, type TranslationMode, type WordMeaning } from "../../types/translation";

const PROVIDER_NAMES: Record<string, string> = {
  ollama: "Ollama (local)", lmstudio: "LM Studio (local)", gemini: "Gemini (cloud)", anthropic: "Claude (cloud)", openai: "OpenAI (cloud)",
  openrouter: "OpenRouter (cloud)", groq: "Groq (cloud)", mistral: "Mistral (cloud)", deepseek: "DeepSeek (cloud)", together: "Together (cloud)", xai: "xAI (cloud)", custom: "Custom",
};
const LOCAL_PROVIDERS = new Set(["ollama", "lmstudio"]);

const EXAMPLE = "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.";

export function TranslatePage({
  settings,
  onOpenSettings,
  onExplore,
  initial = "",
  overlay = false,
  onToggleOverlay,
}: {
  settings: Settings | null;
  onOpenSettings: () => void;
  onExplore: (text: string, meanings?: WordMeaning[]) => void;
  initial?: string;
  overlay?: boolean;
  onToggleOverlay?: () => void;
}) {
  const [text, setText] = useState(initial);
  const [mode, setMode] = useState<TranslationMode>(settings?.default_mode ?? "accurate");
  const [showWords, setShowWords] = useState(settings?.word_meanings ?? true);
  const [isPrivate, setPrivate] = useState(false);
  const [detection, setDetection] = useState<Detection | null>(null);
  const [saved, setSaved] = useState(false);
  const t = useTranslate();
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  useEffect(() => {
    api.listProviders().then(setProviders).catch(() => {});
  }, [settings]);
  const pending = settings ? nextStep(providers.find((p) => p.id === settings.provider), settings) : undefined;

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
    lastAuto.current = text;
    return t.run(text, mode, showWords && !overlay, isPrivate);
  };

  // Live translation: runs on a short pause or at the end of a sentence,
  // never in the middle of a word. Only text typed here is processed.
  const autoOn = (settings?.auto_translate ?? true) && !settings?.paused && !pending;
  const lastEdit = useRef(Date.now());
  const lastAuto = useRef("");
  useEffect(() => {
    if (!autoOn) return;
    const h = setInterval(() => {
      if (shouldTranslate(text, lastAuto.current, Date.now() - lastEdit.current)) translate();
    }, 250);
    return () => clearInterval(h);
  }, [autoOn, text, mode, showWords, isPrivate]);
  // Re-translate the current text when the mode changes.
  useEffect(() => {
    lastAuto.current = "";
  }, [mode]);



  const r = t.result;
  const styledLabel = r?.styled_mode ? MODES.find((m) => m.id === r.styled_mode)?.label + " English" : null;
  const activeModel = settings?.providers[settings.provider]?.model;
  const providerLabel = settings ? `${PROVIDER_NAMES[settings.provider] ?? settings.provider}${activeModel ? ` · ${activeModel}` : ""}` : "—";
  const providerIsLocal = settings ? LOCAL_PROVIDERS.has(settings.provider) : true;

  return (
    <div className={`flex h-full min-h-0 flex-col ${overlay ? "gap-2 p-2 pt-1.5" : "gap-3 p-4"}`}>
      {overlay ? (
        <div data-tauri-drag-region className="flex items-center gap-2 pl-[68px]">
          <ModeSelect compact value={mode} onChange={setMode} />
          <div data-tauri-drag-region className="flex-1" />
          {settings && <OverlayControls settings={settings} compact />}
          <Button variant="primary" className="whitespace-nowrap" onClick={onToggleOverlay} title="Back to the full window">
            Exit
          </Button>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <ModeSelect value={mode} onChange={setMode} />
          <div className="flex-1" />
          <Button onClick={onToggleOverlay} title="Small always-on-top window for meetings — can be hidden from screen sharing">
            ◱ Overlay mode
          </Button>
          <button type="button" onClick={onOpenSettings} className="text-[12px] text-muted hover:text-ink" title="Change provider in Settings">
            Provider: <span className={providerIsLocal ? "text-ok" : "text-crimson"}>{providerLabel}</span>
          </button>
        </div>
      )}

      {pending && (
        <div role="alert" className="flex items-center gap-3 rounded-md bg-warn-bg px-3 py-2 text-warn-text">
          <span className="flex-1">
            Provider setup isn't finished: <strong>{pending.label.charAt(0).toLowerCase() + pending.label.slice(1)}</strong> is still needed before translating.
          </span>
          <Button onClick={onOpenSettings}>Finish setup</Button>
        </div>
      )}

      <div className={`grid min-h-0 flex-1 gap-3 ${overlay ? "grid-cols-1 grid-rows-[2fr_3fr]" : "grid-cols-2"}`}>
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
            placeholder="Start typing in Nepali — English appears on the right as you type.  e.g. Yo error kina aayo?"
            value={text}
            spellCheck={false}
            onChange={(e) => {
              setText(e.target.value);
              lastEdit.current = Date.now();
              if (!e.target.value.trim()) t.reset();
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                translate();
              }
            }}
          />
        </Panel>

        <Panel
          title={
            <span>
              English
              <span className={`ml-2 font-normal normal-case tracking-normal ${autoOn ? "text-ok" : ""}`}>
                {autoOn ? "· Live" : settings?.paused ? "· Paused" : "· Manual"}
              </span>
            </span>
          }
          actions={r && <CopyButton text={mode !== "accurate" && r.styled_english ? r.styled_english : r.accurate_english} label="Copy English" />}
        >
          <div className="flex flex-col gap-3 p-3">
            {t.loading && <Spinner label={r ? "Updating…" : "Translating…"} />}
            <ErrorBanner error={t.error} onRetry={translate} />
            {!r && !t.loading && !t.error && <p className="text-muted">{autoOn ? "English appears here as you type." : "Press Translate or ⌘↩ to see the English translation here."}</p>}
            {r && overlay && <Block label={mode !== "accurate" && r.styled_english ? `${styledLabel}` : "English"} text={mode !== "accurate" && r.styled_english ? r.styled_english : r.accurate_english} big />}
            {r && overlay && r.warnings.map((w) => <Notice key={w}>{w}</Notice>)}
            {r && !overlay && (
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

      {!overlay && r && showWords && r.word_meanings.length > 0 && (
        <Panel title="Word meanings" className="max-h-64" actions={<Button variant="ghost" onClick={() => onExplore(r.original_text, r.word_meanings)}>Open in Word Explorer</Button>}>
          <WordMeaningTable items={r.word_meanings} />
        </Panel>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button variant="primary" onClick={translate} disabled={!text.trim() || t.loading} aria-keyshortcuts="Meta+Enter">
          Translate <span className="opacity-70">⌘↩</span>
        </Button>
        <label className="flex items-center gap-1.5 text-[12px] text-muted" title="Translate automatically while you type">
          <input
            type="checkbox"
            checked={settings?.auto_translate ?? true}
            onChange={(e) => api.updateSettings({ ...settings!, auto_translate: e.target.checked }).catch(() => {})}
            className="accent-[var(--accent)]"
            disabled={!settings}
          />
          Auto-translate as I type
        </label>
        <label hidden={overlay} className="flex items-center gap-1.5 text-[12px] text-muted">
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
        {r && !overlay && (
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
