import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button, ErrorBanner, Notice } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type CoworkerEvent, type Decision, type SessionRow, type Specialist, type Status, type Investigation } from "./api";
import { empty, reduce, type Item, type Transcript } from "./transcript";

const EXAMPLES = [
  "mero downloads folder ma pdf haru khojera summarize gara",
  "yo folder ma bhayeko meeting notes ko professional English summary banaideu",
  "Find the newest text files in my Documents folder.",
];

export function CoworkerPage({ onOpenSettings, start }: { onOpenSettings: () => void; start?: { specialist: Specialist; investigation?: Investigation; prompt?: string; n: number } }) {
  const [status, setStatus] = useState<Status | null>(null);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [transcripts, setTranscripts] = useState<Record<string, Transcript>>({});
  const [input, setInput] = useState("");
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await cw.status());
      setSessions(await cw.sessions());
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

  useEffect(() => endRef.current?.scrollIntoView({ behavior: "smooth" }), [transcripts, active]);

  const open = async (id: string) => {
    setActive(id);
    if (!transcripts[id]) {
      const msgs = await cw.messages(id).catch(() => []);
      setTranscripts((all) => ({ ...all, [id]: { running: false, items: msgs.map((m) => (m.role === "user" ? { kind: "user", text: m.text } : { kind: "assistant", text: m.text, streaming: false })) as Item[] } }));
    }
  };

  const newSession = useCallback(async (specialist: Specialist = "general", investigation?: Investigation) => {
    setBusy(true);
    setError(null);
    try {
      const s = await cw.newSession(specialist, investigation);
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
  }, []);

  const send = useCallback(async (text: string, session?: string | null) => {
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
  }, [active, newSession]);

  // REA Lab and other screens can start a specialist conversation here.
  const started = useRef(0);
  useEffect(() => {
    if (!start || started.current === start.n) return;
    started.current = start.n;
    void (async () => {
      const id = await newSession(start.specialist, start.investigation);
      if (id && start.prompt) await send(start.prompt, id);
    })();
  }, [start, newSession, send]);

  const decide = async (id: string, d: Decision) => {
    await cw.decide(id, d);
  };

  const t = active ? transcripts[active] ?? empty : empty;
  const activeRow = sessions.find((s) => s.id === active);

  return (
    <div className="flex h-full min-h-0">
      <aside className="flex w-56 shrink-0 flex-col gap-3 border-r border-line p-3">
        <Button variant="primary" onClick={() => void newSession()} disabled={busy}>New conversation</Button>
        <section aria-labelledby="folders-h">
          <div className="mb-1 flex items-center justify-between">
            <h2 id="folders-h" className="text-[11px] font-semibold uppercase tracking-wide text-muted">Folders</h2>
            <button className="text-[12px] text-accent hover:underline" onClick={() => void cw.addGrant().then(refresh).catch((e) => setError(toAppError(e)))}>+ Add</button>
          </div>
          {status?.grants.length ? (
            <ul className="space-y-1">
              {status.grants.map((g) => (
                <li key={g.id} className="group flex items-center justify-between gap-1 rounded bg-surface-2 px-2 py-1 text-[12px]" title={g.path}>
                  <span className="truncate">{g.path.split("/").pop() || g.path}</span>
                  <button aria-label={`Remove access to ${g.path}`} className="text-muted opacity-0 group-hover:opacity-100" onClick={() => void cw.revokeGrant(g.id).then(refresh)}>×</button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[12px] text-muted">No folders yet. The coworker can only use folders you add.</p>
          )}
        </section>
        <section className="min-h-0 flex-1 overflow-auto" aria-label="Conversations">
          <h2 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-muted">Conversations</h2>
          <ul className="space-y-0.5">
            {sessions.map((s) => (
              <li key={s.id}>
                <button onClick={() => void open(s.id)} className={`w-full truncate rounded px-2 py-1 text-left text-[12.5px] ${s.id === active ? "bg-accent-soft text-ink" : "text-muted hover:bg-surface-2"}`}>
                  {s.specialist.startsWith("rea") ? "🔍 " : ""}{s.title}
                </button>
              </li>
            ))}
          </ul>
        </section>
      </aside>

      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-line px-5 py-3">
          <div>
            <h1 className="text-[15px] font-semibold">{activeRow?.specialist.startsWith("rea") ? "REA Application Intelligence" : "Coworker"}</h1>
            <p className="text-[12px] text-muted">{status?.model ? `Model: ${status.model}` : "Write in Romanized Nepali, नेपाली or English."}</p>
          </div>
          {t.running && active && <Button onClick={() => void cw.cancel(active)}>Stop</Button>}
        </header>

        <div className="min-h-0 flex-1 overflow-auto px-5 py-4">
          {status && !status.provider_ready && (
            <Notice>
              {status.provider_problem} <button className="underline" onClick={onOpenSettings}>Open Settings</button>
            </Notice>
          )}
          {status && !status.audit_intact && <Notice>The activity log failed its integrity check. See Activity.</Notice>}
          <ErrorBanner error={error} />
          {t.items.length === 0 ? (
            <div className="mx-auto mt-10 max-w-lg text-center">
              <p className="text-[15px] font-medium">K garna sahayog garum?</p>
              <p className="mt-1 text-[13px] text-muted">Ask in your own words. Reads inside your folders run automatically; anything that changes files asks you first.</p>
              <div className="mt-4 space-y-2">
                {EXAMPLES.map((ex) => (
                  <button key={ex} onClick={() => setInput(ex)} className="block w-full rounded-md border border-line px-3 py-2 text-left text-[13px] hover:bg-surface-2">{ex}</button>
                ))}
              </div>
            </div>
          ) : (
            <ol className="mx-auto max-w-3xl space-y-3">
              {t.items.map((it, i) => <ItemView key={i} item={it} onDecide={decide} onAnswer={(id, a) => void cw.answer(id, a)} />)}
            </ol>
          )}
          <div ref={endRef} />
        </div>

        <form className="border-t border-line p-3" onSubmit={(e) => { e.preventDefault(); void send(input); }}>
          <div className="mx-auto flex max-w-3xl gap-2">
            <label htmlFor="cw-input" className="sr-only">Message</label>
            <textarea
              id="cw-input"
              rows={2}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void send(input); } }}
              placeholder="e.g. mero downloads folder ma pdf haru khojera summarize gara"
              className="min-h-[44px] flex-1 resize-none rounded-md border border-line bg-surface px-3 py-2 text-[14px] outline-none focus:border-accent"
              spellCheck={false}
            />
            <Button variant="primary" type="submit" disabled={!input.trim() || t.running}>Send</Button>
          </div>
        </form>
      </section>
    </div>
  );
}

function ItemView({ item, onDecide, onAnswer }: { item: Item; onDecide: (id: string, d: Decision) => void; onAnswer: (id: string, a: string) => void }) {
  switch (item.kind) {
    case "user":
      return <li className="ml-auto max-w-[80%] whitespace-pre-wrap rounded-lg bg-accent-soft px-3 py-2 text-[14px]">{item.text}</li>;
    case "assistant":
      return <li className="whitespace-pre-wrap text-[14px] leading-relaxed">{item.text}{item.streaming && <span className="animate-pulse">▍</span>}</li>;
    case "tool":
      return <li className="text-[12px] text-muted">{item.done ? "✓" : "…"} {label(item.tool)}</li>;
    case "denied":
      return <li className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]"><b>Blocked:</b> {label(item.tool)} — {item.reason}</li>;
    case "error":
      return <li className="rounded-md border border-red-300 px-3 py-2 text-[12.5px] text-red-600 dark:border-red-800 dark:text-red-400" role="alert">{item.text}</li>;
    case "question":
      return <QuestionCard item={item} onAnswer={onAnswer} />;
    case "approval":
      return (
        <li className={`rounded-lg border px-4 py-3 ${item.hardFloor ? "border-amber-400" : "border-accent"}`} aria-live="polite">
          <p className="text-[13px] font-semibold">{item.hardFloor ? "⚠︎ " : ""}Approve: {label(item.tool)}?</p>
          <p className="mt-0.5 text-[12.5px] text-muted">{item.reason}</p>
          <pre className="mt-2 max-h-48 overflow-auto rounded bg-surface-2 p-2 text-[11.5px]">{JSON.stringify(item.args, null, 2)}</pre>
          {item.decision ? (
            <p className="mt-2 text-[12.5px]">{item.decision === "deny" ? "You declined." : item.decision === "session" ? "Allowed for this conversation." : "Allowed once."}</p>
          ) : (
            <div className="mt-2 flex gap-2">
              <Button variant="primary" onClick={() => onDecide(item.id, "once")}>Allow once</Button>
              {item.standingOk && <Button onClick={() => onDecide(item.id, "session")}>Allow for this conversation</Button>}
              <Button onClick={() => onDecide(item.id, "deny")}>Deny</Button>
            </div>
          )}
        </li>
      );
  }
}

function QuestionCard({ item, onAnswer }: { item: Extract<Item, { kind: "question" }>; onAnswer: (id: string, a: string) => void }) {
  const [text, setText] = useState("");
  return (
    <li className="rounded-lg border border-line px-4 py-3">
      <p className="text-[13px] font-medium">{item.question}</p>
      <div className="mt-2 flex flex-wrap gap-2">
        {item.options.map((o) => <Button key={o} onClick={() => onAnswer(item.id, o)}>{o}</Button>)}
        <input aria-label="Your answer" value={text} onChange={(e) => setText(e.target.value)} className="min-w-[200px] flex-1 rounded border border-line bg-surface px-2 text-[13px]" />
        <Button onClick={() => text && onAnswer(item.id, text)}>Reply</Button>
      </div>
    </li>
  );
}

const LABELS: Record<string, string> = {
  "dn.understand_text": "Understanding your Nepali",
  "dn.list_folder": "Listing a folder",
  "dn.search_files": "Searching files",
  "dn.read_file": "Reading a file",
  "dn.read_pdf": "Reading a PDF",
  "dn.write_file": "Writing a file",
};
export const label = (tool: string) => LABELS[tool] ?? (tool.startsWith("rea_") ? `REA · ${tool.split(".").pop()}` : tool);
