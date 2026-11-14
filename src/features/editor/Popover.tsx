import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { Button, CopyButton, ErrorBanner, Notice, Spinner } from "../../components/ui";
import { ModeSelect } from "../../components/ModeSelect";
import { useTranslate } from "../../hooks/useTranslate";
import { useAppearance } from "../../hooks/useAppearance";
import { api, isTauri } from "../../providers/api";
import type { AppError, PendingCapture, Settings, TranslationMode } from "../../types/translation";

/** Compact quick-translate window opened by the global shortcut. */
export function Popover() {
  const [capture, setCapture] = useState<PendingCapture | null>(null);
  const [text, setText] = useState("");
  const [editing, setEditing] = useState(false);
  const [mode, setMode] = useState<TranslationMode>("accurate");
  const [actionError, setActionError] = useState<AppError | null>(null);
  const t = useTranslate();
  const [appearance, setAppearance] = useState<Settings | null>(null);
  useAppearance(appearance);

  const start = useCallback(
    async (p: PendingCapture) => {
      setCapture(p);
      setActionError(null);
      setEditing(false);
      t.reset();
      let m: TranslationMode = "accurate";
      try {
        const s: Settings = await api.getSettings();
        m = s.default_mode;
        setAppearance(s);
      } catch {
        /* default */
      }
      setMode(m);
      setText(p.text ?? "");
      if ((p.status === "ok" || p.status === "clipboard") && p.text) t.run(p.text, m, false);
    },
    [t.run, t.reset],
  );

  useEffect(() => {
    api.getPendingCapture().then((p) => { if (p.seq > 0) start(p); }).catch(() => {});
    if (!isTauri()) return;
    const un = listen<PendingCapture>("popover:capture", (e) => start(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, [start]);

  const close = (restore = true) => api.dismissPopover(restore).catch(() => {});

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const output = t.result ? (mode !== "accurate" && t.result.styled_english ? t.result.styled_english : t.result.accurate_english) : null;
  const status = capture?.status;
  const problem = status && status !== "ok" && status !== "clipboard";

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-xl border border-line bg-surface text-ink">
      <header data-tauri-drag-region className="flex h-9 shrink-0 items-center gap-2 border-b border-line px-3">
        <span data-tauri-drag-region className="text-[12px] font-semibold">DigitsNepal</span>
        {status === "clipboard" && <span className="text-[11px] text-muted">· from clipboard</span>}
        <div data-tauri-drag-region className="flex-1" />
        <ModeSelect compact value={mode} onChange={(m) => { setMode(m); if (text.trim()) t.run(text, m, false); }} />
        <Button variant="ghost" onClick={() => close(true)} aria-label="Cancel (Escape)">
          ✕
        </Button>
      </header>

      <div className="min-h-0 flex-1 space-y-2 overflow-auto p-3">
        {problem && (
          <ProblemNotice
            capture={capture!}
            onUseClipboard={async () => {
              try {
                const c = await api.readClipboardText();
                setCapture({ ...capture!, status: "clipboard", text: c, can_replace: false });
                setText(c);
                t.run(c, mode, false);
              } catch (e) {
                setActionError(e as AppError);
              }
            }}
            onType={() => setEditing(true)}
          />
        )}

        {(text || editing) && (
          <div>
            <div className="mb-0.5 text-[10.5px] font-semibold uppercase tracking-wide text-muted">Original</div>
            {editing ? (
              <textarea
                autoFocus
                aria-label="Edit original text"
                className="h-20 w-full resize-none rounded-md border border-line bg-surface-2 p-2 outline-none"
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                    e.preventDefault();
                    setEditing(false);
                    t.run(text, mode, false);
                  }
                }}
              />
            ) : (
              <p data-selectable className="line-clamp-3 text-muted">{text}</p>
            )}
          </div>
        )}

        {t.loading && <Spinner />}
        <ErrorBanner error={t.error} onRetry={() => t.run(text, mode, false)} />
        {actionError && <div role="alert" className="rounded-md bg-error-bg px-3 py-2 text-[12px] text-error-text">{actionError.message}</div>}
        {output && (
          <div>
            <div className="mb-0.5 text-[10.5px] font-semibold uppercase tracking-wide text-muted">English</div>
            <p data-selectable className="text-[15px] leading-relaxed">{output}</p>
          </div>
        )}
        {t.result?.warnings.map((w) => <Notice key={w}>{w}</Notice>)}
      </div>

      <footer className="flex shrink-0 items-center gap-1.5 border-t border-line px-3 py-2">
        {capture?.can_replace && (
          <Button
            variant="primary"
            disabled={!output}
            title="Replace the selected text in the original app"
            onClick={async () => {
              if (!output) return;
              setActionError(null);
              try {
                await api.replaceSelection(output);
              } catch (e) {
                setActionError(e as AppError);
              }
            }}
          >
            Replace
          </Button>
        )}
        <CopyButton text={output} />
        {editing ? (
          <Button onClick={() => { setEditing(false); t.run(text, mode, false); }} disabled={!text.trim()}>
            Translate ⌘↩
          </Button>
        ) : (
          <Button variant="ghost" onClick={() => setEditing(true)}>
            Edit
          </Button>
        )}
        <div className="flex-1" />
        <Button variant="ghost" onClick={() => { api.openMain().catch(() => {}); close(false); }}>
          Open app
        </Button>
        <Button variant="ghost" onClick={() => close(true)}>
          Cancel
        </Button>
      </footer>
    </div>
  );
}

function ProblemNotice({ capture, onUseClipboard, onType }: { capture: PendingCapture; onUseClipboard: () => void; onType: () => void }) {
  if (capture.status === "permission_required") {
    return (
      <Notice>
        <p>{capture.message}</p>
        <div className="mt-2 flex gap-2">
          <Button onClick={() => api.requestAccessibility()}>Open System Settings</Button>
          <Button variant="ghost" onClick={onType}>Type text instead</Button>
        </div>
      </Notice>
    );
  }
  if (capture.status === "secure_input") return <Notice>{capture.message}</Notice>;
  return (
    <Notice>
      <p>{capture.message ?? "No selected text was found."}</p>
      <div className="mt-2 flex flex-wrap gap-2">
        {capture.clipboard_fallback && (
          <Button onClick={onUseClipboard} title="Reads your clipboard once, only now">
            Use clipboard text (copy it first with ⌘C)
          </Button>
        )}
        <Button variant="ghost" onClick={onType}>Type text instead</Button>
      </div>
    </Notice>
  );
}
