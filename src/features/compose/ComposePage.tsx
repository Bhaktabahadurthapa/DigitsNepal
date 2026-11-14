import { useEffect, useRef, useState } from "react";
import { Button, CopyButton, ErrorBanner, Notice, Panel, Spinner } from "../../components/ui";
import { ModeSelect } from "../../components/ModeSelect";
import { useTranslate } from "../../hooks/useTranslate";
import { shouldTranslate } from "../../lib/compose";
import type { Settings, TranslationMode } from "../../types/translation";

/**
 * Live Compose: a dedicated window for near-real-time translation. This is the
 * safe MVP alternative to system-wide inline conversion — only text typed here
 * is processed, and nothing is inserted into other apps automatically.
 */
export function ComposePage({ settings, onEnable }: { settings: Settings | null; onEnable: () => void }) {
  const [text, setText] = useState("");
  const [mode, setMode] = useState<TranslationMode>(settings?.default_mode ?? "accurate");
  const [paused, setPaused] = useState(false);
  const lastEdit = useRef(Date.now());
  const lastTranslated = useRef("");
  const t = useTranslate();
  const enabled = !!settings?.live_compose && !settings.paused;

  useEffect(() => {
    if (!enabled || paused) return;
    const h = setInterval(() => {
      if (t.loading) return;
      if (shouldTranslate(text, lastTranslated.current, Date.now() - lastEdit.current)) {
        lastTranslated.current = text;
        t.run(text, mode, false);
      }
    }, 300);
    return () => clearInterval(h);
  }, [text, mode, enabled, paused, t.loading]);

  if (!settings?.live_compose) {
    return (
      <div className="mx-auto max-w-lg p-10 text-center">
        <h2 className="text-[16px] font-semibold">Live Compose is off</h2>
        <p className="mt-2 text-muted">Write in Romanized Nepali and see English appear when you pause or finish a sentence. Only text typed in this window is processed.</p>
        <Button variant="primary" className="mt-4" onClick={onEnable}>
          Turn on Live Compose
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <div className="flex items-center gap-2">
        <ModeSelect value={mode} onChange={setMode} />
        <div className="flex-1" />
        <Button onClick={() => setPaused((p) => !p)} aria-pressed={paused} aria-keyshortcuts="Escape">
          {paused ? "Resume" : "Pause"}
        </Button>
      </div>
      {settings.paused && <Notice>Paused from the menu bar. Resume there to continue.</Notice>}
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel title={paused ? "Compose (paused)" : "Compose"}>
          <textarea
            aria-label="Compose in Nepali"
            className="h-full w-full resize-none bg-transparent p-3 text-[15px] leading-relaxed outline-none"
            placeholder="Lekhna suru garnus… (start typing)"
            value={text}
            spellCheck={false}
            onKeyDown={(e) => e.key === "Escape" && setPaused(true)}
            onChange={(e) => {
              setText(e.target.value);
              lastEdit.current = Date.now();
            }}
          />
        </Panel>
        <Panel title="Live English" actions={t.result && <CopyButton text={t.result.styled_english && mode !== "accurate" ? t.result.styled_english : t.result.accurate_english} />}>
          <div className="space-y-2 p-3">
            {t.loading && <Spinner label="Updating…" />}
            <ErrorBanner error={t.error} onRetry={() => t.run(text, mode, false)} />
            {t.result && (
              <p data-selectable className="text-[15px] leading-relaxed">
                {mode !== "accurate" && t.result.styled_english ? t.result.styled_english : t.result.accurate_english}
              </p>
            )}
            {t.result?.warnings.map((w) => <Notice key={w}>{w}</Notice>)}
            {!t.result && !t.loading && <p className="text-muted">The translation updates when you pause or end a sentence.</p>}
          </div>
        </Panel>
      </div>
    </div>
  );
}
