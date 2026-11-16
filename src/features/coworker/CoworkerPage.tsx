import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle, ArrowUp, Ban, Check, ChevronDown, Code, FileText, Folder, FolderOpen, FolderPlus,
  Loader2, PenLine, Plus, Search, ShieldAlert, ShieldCheck, Sparkles, Square, Workflow, X, type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Button, ErrorBanner } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type CoworkerEvent, type Decision, type SessionRow, type Specialist, type Status } from "./api";
import { empty, reduce, type Item, type Transcript } from "./transcript";

const ICONS: Record<string, LucideIcon> = {
  sparkles: Sparkles, "pen-line": PenLine, search: Search, "file-text": FileText, code: Code, workflow: Workflow,
};
const iconFor = (s?: Specialist) => (s && ICONS[s.icon]) || Sparkles;

function timeAgo(ms: number): string {
  const s = Math.max(1, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.round(s / 60)}m`;
  if (s < 86400) return `${Math.round(s / 3600)}h`;
  return `${Math.round(s / 86400)}d`;
}

export function CoworkerPage({ onOpenSettings }: { onOpenSettings: () => void }) {
  const [status, setStatus] = useState<Status | null>(null);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [transcripts, setTranscripts] = useState<Record<string, Transcript>>({});
  const [input, setInput] = useState("");
  const [path, setPath] = useState("");
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [specialists, setSpecialists] = useState<Specialist[]>([]);
  const [pick, setPick] = useState("general");
  const endRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await cw.status());
      setSessions(await cw.sessions());
      setSpecialists(await cw.specialists());
    } catch (e) {
      setError(toAppError(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const un = listen<CoworkerEvent>("coworker:event", (e) => {
      const ev = e.payload;
      setTranscripts((all) => ({ ...all, [ev.session]: reduce(all[ev.session] ?? empty, ev) }));
      if (ev.type === "done") void cw.sessions().then(setSessions);
    });
    return () => void un.then((f) => f());
  }, [refresh]);

  useEffect(() => endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" }), [transcripts, active]);

  const open = async (id: string) => {
    setActive(id);
    if (!transcripts[id]) {
      const msgs = await cw.messages(id).catch(() => []);
      // Approvals left open by an earlier launch are shown again.
      void cw.resume(id).catch((e) => setError(toAppError(e)));
      const used = await cw.usage(id).catch(() => null);
      setTranscripts((all) => ({
        ...all,
        [id]: { running: false, tokens: used ? used.input_tokens + used.output_tokens : 0, items: msgs.map((m) => (m.role === "user" ? { kind: "user", text: m.text } : { kind: "assistant", text: m.text, streaming: false })) as Item[] },
      }));
    }
  };

  const newSession = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const s = await cw.newSession(pick);
      setSessions((all) => [s, ...all]);
      setTranscripts((all) => ({ ...all, [s.id]: empty }));
      setActive(s.id);
      return s.id;
    } catch (e) {
      setError(toAppError(e));
      return null;
    } finally {
      setBusy(false);
    }
  }, [pick]);

  const send = useCallback(
    async (text: string, session?: string | null) => {
      const t = text.trim();
      if (!t) return;
      let id = session ?? active;
      if (!id) id = await newSession();
      if (!id) return;
      setError(null);
      setInput("");
      setTranscripts((all) => ({ ...all, [id!]: { ...(all[id!] ?? empty), items: [...(all[id!] ?? empty).items, { kind: "user", text: t }] } }));
      try {
        await cw.send(id, t);
      } catch (e) {
        setError(toAppError(e));
      }
    },
    [active, newSession],
  );

  const addFolder = async (typed?: string) => {
    setError(null);
    try {
      await cw.addGrant(typed);
      setPath("");
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  const t = active ? transcripts[active] ?? empty : empty;
  const byKey = (k?: string) => specialists.find((s) => s.key === k);
  const activeRow = sessions.find((s) => s.id === active);
  // The specialist shown in the header: the open chat's, or the one picked for a new chat.
  const current = byKey(activeRow ? activeRow.specialist : pick);
  const startNew = () => { setActive(null); setError(null); inputRef.current?.focus(); };

  return (
    <div className="flex h-full min-h-0 bg-bg">
      {/* Left panel: new chat, folders, conversations */}
      <aside className="flex w-64 shrink-0 flex-col gap-4 border-r border-line bg-surface-2 p-3">
        <Button variant="primary" className="w-full" onClick={startNew} disabled={busy}>
          <Plus size={15} aria-hidden /> New chat
        </Button>

        <section aria-labelledby="folders-h" className="rounded-xl border border-line bg-surface p-2.5 shadow-card">
          <div className="mb-2 flex items-center justify-between">
            <h2 id="folders-h" className="flex items-center gap-1.5 text-[12px] font-semibold text-ink">
              <ShieldCheck size={14} className="text-ok" aria-hidden /> Folder access
            </h2>
            <button className="flex h-7 items-center gap-1 rounded-md px-2 text-[12px] font-medium text-accent-fg transition-colors hover:bg-accent-soft" onClick={() => void addFolder()}>
              <FolderPlus size={14} aria-hidden /> Add
            </button>
          </div>
          {status?.grants.length ? (
            <ul className="space-y-1">
              {status.grants.map((g) => (
                <li key={g.id} className="group flex h-8 items-center gap-2 rounded-lg bg-surface px-2 text-[12.5px] shadow-card" title={g.path}>
                  <Folder size={14} className="shrink-0 text-accent-fg" aria-hidden />
                  <span className="min-w-0 flex-1 truncate">{g.path.split("/").pop() || g.path}</span>
                  <button aria-label={`Remove access to ${g.path}`} className="rounded p-1 text-muted opacity-0 transition-opacity hover:bg-error-bg hover:text-error-text focus:opacity-100 group-hover:opacity-100" onClick={() => void cw.revokeGrant(g.id).then(refresh)}>
                    <X size={13} aria-hidden />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[12px] leading-relaxed text-muted">Sahayogi can only use folders you add. Nothing else on your Mac is visible to it.</p>
          )}
          <form className="mt-2" onSubmit={(e) => { e.preventDefault(); if (path.trim()) void addFolder(path); }}>
            <label htmlFor="grant-path" className="sr-only">Folder path</label>
            <input
              id="grant-path"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="Or paste a folder path, then Enter"
              className="h-8 w-full rounded-lg border border-line bg-surface px-2.5 text-[12px] outline-none transition-colors placeholder:text-muted focus:border-accent"
            />
          </form>
        </section>

        <section className="min-h-0 flex-1 overflow-auto" aria-labelledby="chats-h">
          <h2 id="chats-h" className="mb-1.5 px-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Recent chats</h2>
          {sessions.length === 0 ? (
            <p className="px-1 text-[12px] text-muted">No chats yet.</p>
          ) : (
            <ul className="space-y-0.5">
              {sessions.map((s) => {
                const sp = byKey(s.specialist);
                const Icon = iconFor(sp);
                return (
                  <li key={s.id}>
                    <button
                      onClick={() => void open(s.id)}
                      aria-current={s.id === active ? "true" : undefined}
                      className={`flex h-9 w-full items-center gap-2 rounded-lg px-2 text-left text-[12.5px] transition-colors ${s.id === active ? "bg-accent-soft font-medium text-ink" : "text-muted hover:bg-surface-2 hover:text-ink"}`}
                    >
                      <Icon size={14} className="shrink-0" aria-label={sp?.name} />
                      <span className="min-w-0 flex-1 truncate">{s.title}</span>
                      <span className="text-[10.5px] opacity-70">{timeAgo(s.updated_at)}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </aside>

      {/* Conversation */}
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center justify-between border-b border-line bg-surface px-5">
          <div className="flex items-center gap-3">
            <Avatar icon={iconFor(current)} />
            <div>
              <h1 className="text-[14px] font-semibold leading-tight">{current?.name ?? "Sahayogi"}{current && current.key !== "general" && <span className="ml-1.5 text-[12px] font-normal text-muted">{current.nepali}</span>}</h1>
              <p className="flex items-center gap-1.5 text-[11.5px] text-muted">
                <span className={`h-1.5 w-1.5 rounded-full ${t.running ? "animate-pulse bg-accent" : status?.provider_ready ? "bg-ok" : "bg-warn-text"}`} aria-hidden />
                {t.running ? "Working…" : status?.model ? status.model : current?.summary ?? "सहयोगी · your AI work assistant"}
                {!!t.tokens && <span className="ml-1 rounded-full bg-surface-2 px-1.5 text-[10.5px]" title="Tokens used in this chat">{t.tokens.toLocaleString()} tokens</span>}
              </p>
            </div>
          </div>
          {t.running && active && (
            <Button onClick={() => void cw.cancel(active)}>
              <Square size={12} fill="currentColor" aria-hidden /> Stop
            </Button>
          )}
        </header>

        <div className="min-h-0 flex-1 overflow-auto">
          <div className="mx-auto max-w-3xl space-y-3 px-6 py-5">
            {status && !status.provider_ready && (
              <Callout tone="warn" icon={<AlertTriangle size={16} aria-hidden />}>
                {status.provider_problem}{" "}
                <button className="font-medium underline underline-offset-2" onClick={onOpenSettings}>Open Settings</button>
              </Callout>
            )}
            {status && !status.audit_intact && (
              <Callout tone="warn" icon={<ShieldAlert size={16} aria-hidden />}>The activity log failed its integrity check. See Activity.</Callout>
            )}
            <ErrorBanner error={error} title="Something went wrong" note={null} />

            {t.items.length === 0 ? (
              <EmptyState
                specialists={specialists}
                current={current}
                canChoose={!active}
                onChoose={setPick}
                onPick={(text) => { setInput(text); inputRef.current?.focus(); }}
                hasFolders={!!status?.grants.length}
                onAddFolder={() => void addFolder()}
              />
            ) : (
              <ol className="space-y-4" aria-live="polite">
                {t.items.map((it, i) => (
                  <ItemView key={i} item={it} icon={iconFor(current)} who={current?.name ?? "Sahayogi"} onDecide={(id, d) => void cw.decide(id, d)} onAnswer={(id, a) => void cw.answer(id, a)} />
                ))}
              </ol>
            )}
            <div ref={endRef} />
          </div>
        </div>

        <form className="shrink-0 border-t border-line bg-surface px-6 py-3" onSubmit={(e) => { e.preventDefault(); void send(input); }}>
          <div className="mx-auto flex max-w-3xl items-end gap-2 rounded-2xl border border-line bg-surface px-3 py-2 shadow-card transition-colors focus-within:border-accent">
            <label htmlFor="cw-input" className="sr-only">Message {current?.name ?? "Sahayogi"}</label>
            <textarea
              id="cw-input"
              ref={inputRef}
              rows={1}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void send(input); } }}
              placeholder={`${current?.name ?? "Sahayogi"} lai sodhnuhos… (Romanized Nepali, नेपाली or English)`}
              className="max-h-40 min-h-[36px] flex-1 resize-none bg-transparent py-1.5 text-[14px] leading-relaxed outline-none placeholder:text-muted"
              spellCheck={false}
            />
            <button
              type="submit"
              aria-label="Send"
              disabled={!input.trim() || t.running}
              className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent text-on-accent transition-colors hover:bg-accent-hover disabled:bg-surface-2 disabled:text-muted"
            >
              <ArrowUp size={17} strokeWidth={2.4} aria-hidden />
            </button>
          </div>
          <p className="mx-auto mt-1.5 max-w-3xl text-center text-[11px] text-muted">Enter to send · Shift+Enter for a new line · Changes to files always ask you first</p>
        </form>
      </section>
    </div>
  );
}

function Avatar({ size = 32, icon: Icon = Sparkles }: { size?: number; icon?: LucideIcon }) {
  return (
    <div className="flex shrink-0 items-center justify-center rounded-full bg-accent-soft text-accent-fg" style={{ width: size, height: size }} aria-hidden>
      <Icon size={size * 0.5} />
    </div>
  );
}

function Callout({ tone, icon, children }: { tone: "warn" | "info"; icon: ReactNode; children: ReactNode }) {
  return (
    <div className={`flex items-start gap-2.5 rounded-xl px-3.5 py-2.5 text-[12.5px] leading-relaxed ${tone === "warn" ? "bg-warn-bg text-warn-text" : "bg-accent-soft text-ink"}`} role={tone === "warn" ? "alert" : undefined}>
      <span className="mt-0.5 shrink-0">{icon}</span>
      <div>{children}</div>
    </div>
  );
}

function EmptyState({ specialists, current, canChoose, onChoose, onPick, hasFolders, onAddFolder }: {
  specialists: Specialist[]; current?: Specialist; canChoose: boolean; onChoose: (key: string) => void;
  onPick: (t: string) => void; hasFolders: boolean; onAddFolder: () => void;
}) {
  return (
    <div className="fade-in mx-auto max-w-2xl pt-6 text-center">
      <div className="mx-auto w-fit"><Avatar size={52} icon={iconFor(current)} /></div>
      <h2 className="mt-4 text-[20px] font-semibold tracking-tight">K garna sahayog garum?</h2>
      <p className="mt-1.5 text-[13px] text-muted">Ask in your own words. Every specialist reads only the folders you share and asks before changing anything.</p>
      {!hasFolders && (
        <button onClick={onAddFolder} className="mx-auto mt-4 flex h-9 items-center gap-2 rounded-lg border border-dashed border-accent px-4 text-[12.5px] font-medium text-accent-fg transition-colors hover:bg-accent-soft">
          <FolderOpen size={15} aria-hidden /> Share a folder to get started
        </button>
      )}
      {canChoose && specialists.length > 0 && (
        <fieldset className="mt-6 text-left">
          <legend className="mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Choose a specialist</legend>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            {specialists.map((s) => {
              const Icon = iconFor(s);
              const on = s.key === current?.key;
              return (
                <button
                  key={s.key}
                  type="button"
                  aria-pressed={on}
                  onClick={() => onChoose(s.key)}
                  className={`flex flex-col items-start gap-1 rounded-xl border p-3 text-left shadow-card transition-colors ${on ? "border-accent bg-accent-soft" : "border-line bg-surface hover:border-accent"}`}
                >
                  <span className="flex items-center gap-2 text-[13px] font-semibold text-ink">
                    <Icon size={15} className="text-accent-fg" aria-hidden /> {s.name}
                    <span className="text-[11.5px] font-normal text-muted">{s.nepali}</span>
                  </span>
                  <span className="text-[11.5px] leading-snug text-muted">{s.summary}</span>
                </button>
              );
            })}
          </div>
        </fieldset>
      )}
      {current && (
        <div className="mt-5 grid gap-2 text-left">
          {current.examples.map((text) => (
            <button key={text} onClick={() => onPick(text)} className="group flex items-center gap-3 rounded-xl border border-line bg-surface p-3 shadow-card transition-colors hover:border-accent hover:bg-accent-soft">
              <ArrowUp size={14} className="shrink-0 rotate-45 text-accent-fg" aria-hidden />
              <span className="text-[13px] text-ink">{text}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ItemView({ item, icon, who, onDecide, onAnswer }: { item: Item; icon: LucideIcon; who: string; onDecide: (id: string, d: Decision) => void; onAnswer: (id: string, a: string) => void }) {
  switch (item.kind) {
    case "user":
      return (
        <li className="fade-in flex justify-end">
          <div data-selectable className="max-w-[80%] whitespace-pre-wrap rounded-2xl rounded-br-md bg-accent px-4 py-2.5 text-[14px] leading-relaxed text-on-accent">{item.text}</div>
        </li>
      );
    case "assistant":
      return (
        <li className="fade-in flex gap-3">
          <Avatar size={28} icon={icon} />
          <div data-selectable className="min-w-0 flex-1 whitespace-pre-wrap pt-0.5 text-[14px] leading-relaxed text-ink">
            {item.text}
            {item.streaming && <span className="ml-0.5 inline-block h-4 w-1.5 animate-pulse rounded-sm bg-accent align-middle" aria-hidden />}
          </div>
        </li>
      );
    case "tool":
      return (
        <li className="ml-10 flex items-center gap-2 text-[12px] text-muted">
          <span className={`flex h-5 w-5 items-center justify-center rounded-full ${item.done ? "bg-ok-bg text-ok" : "bg-accent-soft text-accent-fg"}`}>
            {item.done ? <Check size={11} strokeWidth={3} aria-hidden /> : <Loader2 size={11} className="animate-spin" aria-hidden />}
          </span>
          {label(item.tool)}
        </li>
      );
    case "denied":
      return (
        <li className="ml-10 flex items-start gap-2 rounded-xl border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
          <Ban size={14} className="mt-0.5 shrink-0 text-error-text" aria-hidden />
          <span><span className="font-medium">Blocked: {label(item.tool)}.</span> <span className="text-muted">{item.reason}</span></span>
        </li>
      );
    case "error":
      return (
        <li className="ml-10 flex items-start gap-2 rounded-xl bg-error-bg px-3 py-2 text-[12.5px] text-error-text" role="alert">
          <AlertTriangle size={14} className="mt-0.5 shrink-0" aria-hidden /> {item.text}
        </li>
      );
    case "question":
      return <QuestionCard item={item} onAnswer={onAnswer} />;
    case "approval":
      return <ApprovalCard item={item} who={who} onDecide={onDecide} />;
  }
}

function ApprovalCard({ item, who, onDecide }: { item: Extract<Item, { kind: "approval" }>; who: string; onDecide: (id: string, d: Decision) => void }) {
  const [open, setOpen] = useState(false);
  const path = typeof item.args === "object" && item.args && "path" in item.args ? String((item.args as { path: unknown }).path) : null;
  return (
    <li className={`fade-in ml-10 overflow-hidden rounded-xl border bg-surface shadow-card ${item.hardFloor ? "border-warn-text/40" : "border-accent"}`}>
      <div className="flex items-start gap-3 p-4">
        <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${item.hardFloor ? "bg-warn-bg text-warn-text" : "bg-accent-soft text-accent-fg"}`}>
          {item.hardFloor ? <ShieldAlert size={18} aria-hidden /> : <ShieldCheck size={18} aria-hidden />}
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[13.5px] font-semibold">{who} wants to: {label(item.tool).toLowerCase()}</p>
          {path && <p className="mt-0.5 truncate font-mono text-[11.5px] text-muted" title={path}>{path}</p>}
          <p className="mt-1 text-[12.5px] text-muted">{item.reason}</p>
          <button onClick={() => setOpen((o) => !o)} aria-expanded={open} className="mt-1.5 flex items-center gap-1 text-[12px] font-medium text-accent-fg">
            <ChevronDown size={13} className={`transition-transform ${open ? "rotate-180" : ""}`} aria-hidden /> {open ? "Hide details" : "Show details"}
          </button>
          {open && <pre data-selectable className="mt-2 max-h-56 overflow-auto rounded-lg bg-surface-2 p-2.5 text-[11.5px] leading-relaxed">{JSON.stringify(item.args, null, 2)}</pre>}
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2 border-t border-line bg-surface-2 px-4 py-2.5">
        {item.decision ? (
          <span className={`flex items-center gap-1.5 text-[12.5px] font-medium ${item.decision === "deny" ? "text-error-text" : "text-ok"}`}>
            {item.decision === "deny" ? <Ban size={14} aria-hidden /> : <Check size={14} aria-hidden />}
            {item.decision === "deny" ? "You declined" : item.decision === "session" ? "Allowed for this chat" : "Allowed once"}
          </span>
        ) : (
          <>
            <Button variant="primary" onClick={() => onDecide(item.id, "once")}><Check size={14} aria-hidden /> Allow once</Button>
            {item.standingOk && <Button onClick={() => onDecide(item.id, "session")}>Allow for this chat</Button>}
            <Button variant="danger" onClick={() => onDecide(item.id, "deny")}>Deny</Button>
            {item.hardFloor && <span className="ml-auto text-[11.5px] text-warn-text">Always needs your approval</span>}
          </>
        )}
      </div>
    </li>
  );
}

function QuestionCard({ item, onAnswer }: { item: Extract<Item, { kind: "question" }>; onAnswer: (id: string, a: string) => void }) {
  const [text, setText] = useState("");
  return (
    <li className="fade-in ml-10 rounded-xl border border-line bg-surface p-4 shadow-card">
      <p className="text-[13.5px] font-medium">{item.question}</p>
      <div className="mt-2.5 flex flex-wrap gap-2">
        {item.options.map((o) => <Button key={o} onClick={() => onAnswer(item.id, o)}>{o}</Button>)}
      </div>
      <form className="mt-2.5 flex gap-2" onSubmit={(e) => { e.preventDefault(); if (text.trim()) onAnswer(item.id, text); }}>
        <label htmlFor={`q-${item.id}`} className="sr-only">Your answer</label>
        <input id={`q-${item.id}`} value={text} onChange={(e) => setText(e.target.value)} placeholder="Type your answer" className="h-8 min-w-0 flex-1 rounded-lg border border-line bg-surface px-2.5 text-[13px] outline-none focus:border-accent" />
        <Button type="submit" variant="primary">Reply</Button>
      </form>
    </li>
  );
}

const LABELS: Record<string, string> = {
  "translation.explain": "Understanding your Nepali",
  "translation.translate": "Translating",
  "files.granted_folders": "Checking your shared folders",
  "files.list": "Listing a folder",
  "files.search": "Searching files",
  "files.read": "Reading a file",
  "files.read_pdf": "Reading a PDF",
  "files.create": "Write a file",
  "files.move": "Move a file",
  // Conversations from earlier versions
  "dn.understand_text": "Understanding your Nepali",
  "dn.granted_folders": "Checking your shared folders",
  "dn.list_folder": "Listing a folder",
  "dn.search_files": "Searching files",
  "dn.read_file": "Reading a file",
  "dn.read_pdf": "Reading a PDF",
  "dn.write_file": "Write a file",
};
export const label = (tool: string) => LABELS[tool] ?? tool;

