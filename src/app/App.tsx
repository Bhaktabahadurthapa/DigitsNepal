import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Activity, BookOpen, Clock, Keyboard, Languages, ScanSearch, Settings as SettingsIcon, Sparkles, type LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ActivityPage } from "../features/coworker/ActivityPage";
import { CoworkerPage } from "../features/coworker/CoworkerPage";
import { ReaLabPage } from "../features/coworker/ReaLabPage";
import type { Investigation } from "../features/coworker/api";
import { WordExplorer } from "../features/dictionary/WordExplorer";
import { HistoryPage } from "../features/history/HistoryPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TranslatePage } from "../features/translation/TranslatePage";
import { useSettings } from "../hooks/useSettings";
import { useAppearance } from "../hooks/useAppearance";
import { isTauri } from "../providers/api";
import type { WordMeaning } from "../types/translation";

type Tab = "translate" | "coworker" | "rea" | "activity" | "explorer" | "history" | "settings";
const TABS: { id: Tab; label: string; key: string }[] = [
  { id: "translate", label: "Translate", key: "1" },
  { id: "coworker", label: "Sahayogi", key: "2" },
  { id: "rea", label: "App Insights", key: "3" },
  { id: "activity", label: "Activity", key: "4" },
  { id: "explorer", label: "Word Explorer", key: "5" },
  { id: "history", label: "History", key: "6" },
  { id: "settings", label: "Settings", key: "," },
];
const NAV_GROUPS: { title: string; tabs: Tab[] }[] = [
  { title: "Language", tabs: ["translate", "explorer", "history"] },
  { title: "Assistant", tabs: ["coworker", "rea", "activity"] },
  { title: "App", tabs: ["settings"] },
];
const ICONS: Record<Tab, LucideIcon> = {
  translate: Languages,
  coworker: Sparkles,
  rea: ScanSearch,
  activity: Activity,
  explorer: BookOpen,
  history: Clock,
  settings: SettingsIcon,
};

export function App() {
  const [tab, setTab] = useState<Tab>("translate");
  const [coworkerStart, setCoworkerStart] = useState<{ specialist: "rea"; investigation: Investigation; prompt: string; n: number } | undefined>();
  const [explore, setExplore] = useState<{ text: string; meanings?: WordMeaning[] }>({ text: "" });
  const [reuse, setReuse] = useState<{ text: string; n: number } | null>(null);
  const { settings, update, error } = useSettings();
  useAppearance(settings);

  // Overlay mode: compact, always on top, on every Space. Combined with
  // "Hide from screen sharing" it stays private during meetings.
  const [overlay, setOverlay] = useState(false);
  const savedSize = useRef<{ w: number; h: number } | null>(null);
  const toggleOverlay = async () => {
    const next = !overlay;
    setTab("translate");
    setOverlay(next);
    if (!isTauri()) return;
    const win = getCurrentWindow();
    try {
      if (next) {
        const sz = await win.innerSize();
        const scale = window.devicePixelRatio || 1;
        savedSize.current = { w: sz.width / scale, h: sz.height / scale };
        await win.setAlwaysOnTop(true);
        await win.setVisibleOnAllWorkspaces(true);
        await win.setSize(new LogicalSize(460, 520));
      } else {
        await win.setAlwaysOnTop(false);
        await win.setVisibleOnAllWorkspaces(false);
        const s = savedSize.current ?? { w: 1080, h: 720 };
        await win.setSize(new LogicalSize(s.w, s.h));
      }
    } catch {
      /* window APIs unavailable — layout still switches */
    }
  };

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
      <nav aria-label="Main" hidden={overlay} className="flex w-52 shrink-0 flex-col border-r border-line bg-sidebar text-side-text">
        <div data-tauri-drag-region className="h-11" />
        <div className="flex items-center gap-2.5 px-4 pb-5">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent text-[15px] font-semibold text-on-accent deva" aria-hidden>अ</div>
          <div className="min-w-0">
            <div className="text-[14px] font-semibold tracking-tight">DigitsNepal</div>
            <div className="truncate text-[10.5px] text-side-muted">Every Nepali Word Has Meaning.</div>
          </div>
        </div>
        {NAV_GROUPS.map((g) => (
          <div key={g.title} className="mb-4 px-2">
            <div className="px-2.5 pb-1.5 text-[10.5px] font-semibold uppercase tracking-[0.08em] text-side-muted">{g.title}</div>
            <ul className="space-y-0.5">
              {g.tabs.map((id) => {
                const t = TABS.find((x) => x.id === id)!;
                const Icon = ICONS[id];
                const active = tab === id;
                return (
                  <li key={id}>
                    <button
                      type="button"
                      aria-current={active ? "page" : undefined}
                      onClick={() => setTab(id)}
                      className={`group flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] transition-colors duration-150 ${active ? "bg-side-active font-medium text-side-text" : "text-side-text hover:bg-side-hover"}`}
                    >
                      <Icon size={16} strokeWidth={1.9} className={active ? "text-accent" : "text-side-muted"} aria-hidden />
                      <span className="flex-1">{t.label}</span>
                      <span className="text-[10.5px] text-side-muted opacity-0 transition-opacity group-hover:opacity-100">⌘{t.key}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
        <div className="flex-1" />
        <div className="m-2 rounded-lg border border-line bg-surface p-3 text-[11px] text-side-muted">
          {settings && (
            <>
              <div className="flex items-center gap-1.5">
                <span className={`h-1.5 w-1.5 rounded-full ${settings.provider ? "bg-ok" : "bg-side-muted"}`} aria-hidden />
                <span className="truncate">AI: <span className="text-side-text">{settings.provider}</span></span>
              </div>
              <div className="mt-1 flex items-center gap-1.5">
                <Keyboard size={12} aria-hidden />
                Shortcut <span className="font-mono text-side-text">{settings.shortcut_enabled && !settings.paused ? shortcut : "off"}</span>
              </div>
            </>
          )}
          {!isTauri() && <div className="mt-1 text-error-text">Browser preview with demo data. Run the desktop app for full features.</div>}
        </div>
      </nav>

      <main className="min-w-0 flex-1">
        <div data-tauri-drag-region className={overlay ? "h-1.5" : "h-3"} />
        <div className={overlay ? "h-[calc(100%-0.375rem)]" : "h-[calc(100%-0.75rem)]"}>
          {tab === "translate" && <TranslatePage key={reuse?.n ?? 0} overlay={overlay} onToggleOverlay={toggleOverlay} settings={settings} onOpenSettings={() => setTab("settings")} onExplore={(text, meanings) => { setExplore({ text, meanings }); setTab("explorer"); }} {...(reuse ? { initial: reuse.text } : {})} />}
          {tab === "coworker" && <CoworkerPage onOpenSettings={() => setTab("settings")} {...(coworkerStart ? { start: coworkerStart } : {})} />}
          {tab === "rea" && <ReaLabPage onStart={(investigation, prompt) => { setCoworkerStart({ specialist: "rea", investigation, prompt, n: Date.now() }); setTab("coworker"); }} />}
          {tab === "activity" && <ActivityPage />}
          {tab === "explorer" && <WordExplorer initialText={explore.text} initialMeanings={explore.meanings} />}
          {tab === "history" && <HistoryPage settings={settings} onEnable={() => update({ history_enabled: true })} onReuse={(text) => { setReuse({ text, n: Date.now() }); setTab("translate"); }} />}
          {tab === "settings" && <SettingsPage settings={settings} update={update} error={error} />}
        </div>
      </main>
    </div>
  );
}
