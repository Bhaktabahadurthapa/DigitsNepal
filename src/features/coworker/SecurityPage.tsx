import { Ban, Check, Cpu, Globe, Hand, OctagonX, RefreshCw, ShieldAlert, ShieldCheck, Undo2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button, ErrorBanner } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type PendingItem, type SecurityStatus, type Standing } from "./api";
import { label } from "./CoworkerPage";

const RULES: { cls: string; what: string; rule: string }[] = [
  { cls: "Read", what: "List, search and read files; translate", rule: "Runs inside folders you shared; anything else is refused" },
  { cls: "Change files", what: "Create, move, save documents and automations", rule: "Asks you; existing files are backed up; nothing is deleted" },
  { cls: "Internet", what: "Web search and pages, GitHub, connected apps", rule: "Asks you; private-network addresses are always refused" },
  { cls: "Run code", what: "Programs in the macOS sandbox", rule: "Off unless you turn it on; asks every time" },
  { cls: "Outside your Mac", what: "Send, post, pay, delete in other apps", rule: "Always asks, never “for this chat”" },
];

function Switch({ icon, title, text, on, danger, disabled, onChange }: { icon: ReactNode; title: string; text: ReactNode; on: boolean; danger?: boolean; disabled?: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className={`flex items-start gap-3 rounded-xl border bg-surface p-4 shadow-card ${on && danger ? "border-warn-text/40 bg-warn-bg" : "border-line"} ${disabled ? "opacity-60" : ""}`}>
      <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${on && danger ? "bg-surface text-warn-text" : "bg-accent-soft text-accent-fg"}`}>{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block text-[13.5px] font-semibold">{title}</span>
        <span className="mt-0.5 block text-[12px] leading-relaxed text-muted">{text}</span>
      </span>
      <input type="checkbox" role="switch" className="mt-1 h-4 w-4 accent-[var(--accent)]" checked={on} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}

export function SecurityPage({ onOpenChat }: { onOpenChat: () => void }) {
  const [sec, setSec] = useState<SecurityStatus | null>(null);
  const [pending, setPending] = useState<PendingItem[]>([]);
  const [standing, setStanding] = useState<Standing[]>([]);
  const [error, setError] = useState<AppError | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const load = async () => {
    try {
      setSec(await cw.security());
      setPending(await cw.pending());
      setStanding(await cw.standing());
    } catch (e) {
      setError(toAppError(e));
    }
  };
  useEffect(() => {
    void load();
    const t = setInterval(() => void load(), 4000);
    return () => clearInterval(t);
  }, []);

  const act = async (f: () => Promise<unknown>, msg?: string) => {
    setError(null);
    try {
      await f();
      if (msg) setNote(msg);
      await load();
    } catch (e) {
      setError(toAppError(e));
    }
  };
  const set = (patch: Partial<SecurityStatus>) =>
    sec && act(() => cw.setSecurity({ paused: sec.paused, network_off: sec.network_off, code_enabled: sec.code_enabled, auto_web: sec.auto_web, auto_files: sec.auto_files, ...patch }));
  const level = !sec ? "" : sec.auto_files && sec.auto_web ? "free" : sec.auto_web ? "research" : "ask";

  const approvals = pending.filter((p) => p.kind === "approval");

  return (
    <div className="h-full overflow-auto bg-bg">
      <div className="mx-auto max-w-4xl space-y-6 p-6">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-[20px] font-semibold tracking-tight">Security Center</h1>
            <p className="mt-0.5 text-[13px] text-muted">Your switches here override everything Sahayogi does. Only you can change them; the assistant has no tool that can.</p>
          </div>
          <Button onClick={() => void load()}><RefreshCw size={14} aria-hidden /> Refresh</Button>
        </div>
        <ErrorBanner error={error} title="Something went wrong" note={null} />
        {note && <p className="rounded-xl bg-accent-soft px-3.5 py-2 text-[12.5px]" role="status">{note}</p>}

        {sec && (
          <section aria-labelledby="sw-h" className="space-y-2">
            <h2 id="sw-h" className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">How much Sahayogi does on its own</h2>
            <div role="radiogroup" aria-label="Autonomy" className="grid gap-2 sm:grid-cols-3">
              {([
                ["ask", "Ask first", "Asks before web research and before saving files.", { auto_web: false, auto_files: false }],
                ["research", "Research freely", "Searches and reads the web on its own; asks before saving files.", { auto_web: true, auto_files: false }],
                ["free", "Work freely", "Researches and saves or organizes files in your shared folders on its own (backups kept, nothing deleted).", { auto_web: true, auto_files: true }],
              ] as const).map(([id, title, text, patch]) => (
                <button
                  key={id}
                  role="radio"
                  aria-checked={level === id}
                  disabled={sec.network_off && id !== "ask"}
                  onClick={() => void set(patch)}
                  className={`rounded-xl border p-3 text-left shadow-card transition-colors disabled:opacity-50 ${level === id ? "border-accent bg-accent-soft" : "border-line bg-surface hover:border-accent"}`}
                >
                  <span className="block text-[13px] font-semibold">{title}</span>
                  <span className="mt-1 block text-[11.5px] leading-snug text-muted">{text}</span>
                </button>
              ))}
            </div>
            <p className="text-[11.5px] text-muted">In every mode, these always ask you: anything outside your Mac (posting, sending, paying), connected apps, running code, and saving automations. Files outside your shared folders and password files are never touched.</p>
            <h2 className="pt-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Switches</h2>
            <Switch icon={<Hand size={17} aria-hidden />} danger title={sec.paused ? "Sahayogi is paused" : "Pause Sahayogi"} on={sec.paused} onChange={(v) => void set({ paused: v })}
              text="Every tool call is refused, even reading your folders. Chats can still answer from what they already know." />
            <Switch icon={<Globe size={17} aria-hidden />} title="Allow internet tools" on={!sec.network_off} onChange={(v) => void set({ network_off: !v })}
              text="Web search, web pages, GitHub and connected apps. When off, nothing Sahayogi does leaves your Mac except your chat with the AI model." />
            <Switch icon={<Cpu size={17} aria-hidden />} title="Run code in the sandbox" on={sec.code_enabled} disabled={!sec.sandbox_available} onChange={(v) => void set({ code_enabled: v })}
              text={sec.sandbox_available
                ? <>Lets Prabidhi run short programs in a macOS sandbox with no internet and no access to your files or apps. You approve every run. Found: {[sec.python && "Python", sec.javascript && "JavaScript", "shell"].filter(Boolean).join(", ")}.</>
                : "The macOS sandbox is not available on this Mac."} />
            <div className="flex items-center justify-between gap-3 rounded-xl border border-line bg-surface p-4 shadow-card">
              <span className="flex items-center gap-3">
                <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-error-bg text-error-text"><OctagonX size={17} aria-hidden /></span>
                <span>
                  <span className="block text-[13.5px] font-semibold">Stop all chats now</span>
                  <span className="block text-[12px] text-muted">Cancels every running chat and declines everything waiting for approval.</span>
                </span>
              </span>
              <Button variant="danger" onClick={() => void act(async () => { const n = await cw.stopAll(); setNote(`Stopped ${n} chat${n === 1 ? "" : "s"}.`); })}>Stop all</Button>
            </div>
          </section>
        )}

        <section aria-labelledby="wait-h" className="space-y-2">
          <h2 id="wait-h" className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Waiting for your approval ({approvals.length})</h2>
          {approvals.length === 0 ? (
            <p className="rounded-xl border border-dashed border-line p-4 text-center text-[12.5px] text-muted">Nothing is waiting.</p>
          ) : (
            approvals.map((p) => (
              <div key={p.id} className={`rounded-xl border bg-surface p-4 shadow-card ${p.hard_floor ? "border-warn-text/40" : "border-line"}`}>
                <div className="flex items-start gap-3">
                  {p.hard_floor ? <ShieldAlert size={18} className="mt-0.5 shrink-0 text-warn-text" aria-hidden /> : <ShieldCheck size={18} className="mt-0.5 shrink-0 text-accent-fg" aria-hidden />}
                  <div className="min-w-0 flex-1">
                    <p className="text-[13.5px] font-semibold">{label(p.tool)}</p>
                    <p className="mt-0.5 whitespace-pre-line text-[12.5px] text-muted">{p.reason}</p>
                    <pre data-selectable className="mt-2 max-h-72 overflow-auto rounded-lg bg-surface-2 p-2.5 text-[11.5px] leading-relaxed">{JSON.stringify(p.preview, null, 2)}</pre>
                  </div>
                </div>
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  {p.live ? (
                    <>
                      <Button variant="primary" onClick={() => void act(() => cw.decide(p.id, "once"))}><Check size={14} aria-hidden /> Allow once</Button>
                      {p.standing_ok && <Button onClick={() => void act(() => cw.decide(p.id, "session"))}>Allow for this chat</Button>}
                      <Button variant="danger" onClick={() => void act(() => cw.decide(p.id, "deny"))}><Ban size={14} aria-hidden /> Deny</Button>
                    </>
                  ) : (
                    <>
                      <span className="text-[12px] text-muted">Waiting since an earlier launch.</span>
                      <Button onClick={onOpenChat}>Open the chat to answer</Button>
                      <Button variant="danger" onClick={() => void act(() => cw.cancel(p.session), "Declined and stopped that chat.")}>Decline</Button>
                    </>
                  )}
                </div>
              </div>
            ))
          )}
        </section>

        <section aria-labelledby="st-h" className="space-y-2">
          <div className="flex items-end justify-between">
            <h2 id="st-h" className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Allowed for a chat ({standing.length})</h2>
            {standing.length > 0 && <Button onClick={() => void act(() => cw.revokeStanding(), "All chat permissions revoked.")}><Undo2 size={13} aria-hidden /> Revoke all</Button>}
          </div>
          {standing.length === 0 ? (
            <p className="rounded-xl border border-dashed border-line p-4 text-center text-[12.5px] text-muted">No chat has a standing permission. Choosing “Allow for this chat” adds one here.</p>
          ) : (
            <ul className="divide-y divide-line rounded-xl border border-line bg-surface shadow-card">
              {standing.map((s) => (
                <li key={`${s.session}-${s.tool}`} className="flex items-center gap-3 px-4 py-2.5">
                  <div className="min-w-0 flex-1">
                    <p className="text-[13px] font-medium">{label(s.tool)}</p>
                    <p className="truncate text-[11.5px] text-muted">in “{s.title || s.session}” · since {new Date(s.created_at).toLocaleString()}</p>
                  </div>
                  <Button onClick={() => void act(() => cw.revokeStanding(s.session, s.tool))}>Revoke</Button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section aria-labelledby="rules-h" className="space-y-2">
          <h2 id="rules-h" className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">How Sahayogi decides</h2>
          <div className="overflow-hidden rounded-xl border border-line bg-surface shadow-card">
            <table className="w-full text-left text-[12.5px]">
              <thead className="bg-surface-2 text-[11.5px] text-muted">
                <tr><th className="px-4 py-2 font-medium">Kind</th><th className="px-4 py-2 font-medium">Examples</th><th className="px-4 py-2 font-medium">Rule</th></tr>
              </thead>
              <tbody className="divide-y divide-line">
                {RULES.map((r) => (
                  <tr key={r.cls}><td className="px-4 py-2 font-medium">{r.cls}</td><td className="px-4 py-2 text-muted">{r.what}</td><td className="px-4 py-2">{r.rule}</td></tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="text-[11.5px] text-muted">Every decision, approval and switch change is written to the tamper-evident log under Activity.</p>
        </section>
      </div>
    </div>
  );
}
