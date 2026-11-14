import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { WordExplorer } from "../features/dictionary/WordExplorer";
import { HistoryPage } from "../features/history/HistoryPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TranslatePage } from "../features/translation/TranslatePage";
import { useSettings } from "../hooks/useSettings";
import { isTauri } from "../providers/api";
import type { WordMeaning } from "../types/translation";

type Tab = "translate" | "explorer" | "history" | "settings";
const TABS: { id: Tab; label: string; key: string }[] = [
  { id: "translate", label: "Translate", key: "1" },
  { id: "explorer", label: "Word Explorer", key: "2" },
  { id: "history", label: "History", key: "3" },
  { id: "settings", label: "Settings", key: "," },
];

export function App() {
  const [tab, setTab] = useState<Tab>("translate");
  const [explore, setExplore] = useState<{ text: string; meanings?: WordMeaning[] }>({ text: "" });
  const [reuse, setReuse] = useState<{ text: string; n: number } | null>(null);
  const { settings, update, error } = useSettings();

  useEffect(() => {
    if (!isTauri()) return;
    const un = listen<string>("navigate", (e) => TABS.some((t) => t.id === e.payload) && setTab(e.payload as Tab));
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey) return;
      const t = TABS.find((x) => x.key === e.key);
      if (t) {
        e.preventDefault();
        setTab(t.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const shortcut = settings?.shortcut.replace("Control", "⌃").replace("Alt", "⌥").replace("Shift", "⇧").replace("Command", "⌘").replaceAll("+", "");

  return (
    <div className="flex h-full">
      <nav aria-label="Main" className="flex w-48 shrink-0 flex-col border-r border-line bg-sidebar">
        <div data-tauri-drag-region className="h-12" />
        <div className="px-4 pb-4">
          <div className="text-[15px] font-semibold tracking-tight">DigitsNepal</div>
          <div className="text-[11px] text-muted">Every Nepali Word Has Meaning.</div>
        </div>
        <ul className="space-y-0.5 px-2">
          {TABS.map((t) => (
            <li key={t.id}>
              <button
                type="button"
                aria-current={tab === t.id ? "page" : undefined}
                onClick={() => setTab(t.id)}
                className={`flex w-full items-center justify-between rounded-md px-2.5 py-1.5 text-left text-[13px] transition-colors ${tab === t.id ? "bg-accent-soft font-medium text-ink" : "text-muted hover:bg-surface-2 hover:text-ink"}`}
              >
                {t.label}
                <span className="text-[10.5px] opacity-60">⌘{t.key}</span>
              </button>
            </li>
          ))}
        </ul>
        <div className="flex-1" />
        <div className="space-y-1 p-4 text-[11px] text-muted">
          {settings && (
            <>
              <div>
                Shortcut: <span className="font-mono">{settings.shortcut_enabled && !settings.paused ? shortcut : "off"}</span>
              </div>
              <div>Provider: {settings.provider}</div>
            </>
          )}
          {!isTauri() && <div className="text-crimson">Browser preview with demo data — not real translations. Run the desktop app for full features.</div>}
        </div>
      </nav>

      <main className="min-w-0 flex-1">
        <div data-tauri-drag-region className="h-3" />
        <div className="h-[calc(100%-0.75rem)]">
          {tab === "translate" && <TranslatePage key={reuse?.n ?? 0} settings={settings} onOpenSettings={() => setTab("settings")} onExplore={(text, meanings) => { setExplore({ text, meanings }); setTab("explorer"); }} {...(reuse ? { initial: reuse.text } : {})} />}
          {tab === "explorer" && <WordExplorer initialText={explore.text} initialMeanings={explore.meanings} />}
          {tab === "history" && <HistoryPage settings={settings} onEnable={() => update({ history_enabled: true })} onReuse={(text) => { setReuse({ text, n: Date.now() }); setTab("translate"); }} />}
          {tab === "settings" && <SettingsPage settings={settings} update={update} error={error} />}
        </div>
      </main>
    </div>
  );
}
