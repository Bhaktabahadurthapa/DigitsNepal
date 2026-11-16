import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle, ArrowUp, Ban, Briefcase, Calendar, Check, CheckCircle2, Circle, CircleDashed, Clock, Coins,
  ExternalLink, Loader2, MapPin, MessageSquare, Package, Pause, Play, RotateCcw, ShieldCheck, Sparkles, Wrench, X, XCircle,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Button, ErrorBanner } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type CoworkerEvent, type PendingItem, type PlanStep, type TaskDetail, type TaskMetrics, type TaskPreview, type TaskRow } from "./api";
import { label } from "./CoworkerPage";

const EXAMPLES: { icon: typeof Briefcase; title: string; text: string }[] = [
  { icon: Wrench, title: "Local service", text: "Malai bholi ghar safa garne service chahiyo. Ramro review bhako ra sasto khojera book gardinu." },
  { icon: Briefcase, title: "Job search", text: "Mero CV anusar ramro jagir khojera apply garna help gara." },
  { icon: Package, title: "Supplier deal", text: "Malai restaurant ko lagi affordable packaging supplier khojera best deal arrange gardinu." },
];

const CATEGORY: Record<string, [string, string]> = {
  job_search: ["जागिर खोजी", "Job search"],
  local_service: ["स्थानीय सेवा", "Local service"],
  supplier_sourcing: ["आपूर्तिकर्ता खोजी", "Supplier sourcing"],
  appointment: ["अपोइन्टमेन्ट", "Appointment"],
  travel: ["यात्रा", "Travel"],
  shopping: ["किनमेल", "Shopping"],
  communication: ["इमेल / सन्देश", "Email & messages"],
  scheduling: ["तालिका", "Scheduling"],
  documents: ["कागजात", "Documents"],
  research: ["अनुसन्धान", "Research"],
  translation: ["अनुवाद", "Translation"],
  general: ["सामान्य", "General"],
};

const STATE: Record<string, [string, string]> = {
  CREATED: ["Created", "bg-surface-2 text-muted"],
  PLANNING: ["Planning", "bg-accent-soft text-accent-fg"],
  READY: ["Ready", "bg-accent-soft text-accent-fg"],
  RUNNING: ["Working", "bg-accent-soft text-accent-fg"],
  NEEDS_CLARIFICATION: ["Needs your answer", "bg-warn-bg text-warn-text"],
  WAITING_FOR_APPROVAL: ["Needs approval", "bg-warn-bg text-warn-text"],
  WAITING_FOR_EXTERNAL_RESPONSE: ["Waiting for reply", "bg-surface-2 text-muted"],
  RETRYING: ["Retrying", "bg-warn-bg text-warn-text"],
  BLOCKED: ["Waiting for you", "bg-warn-bg text-warn-text"],
  COMPLETED: ["Done", "bg-ok-bg text-ok"],
  FAILED: ["Failed", "bg-error-bg text-error-text"],
  CANCELLED: ["Cancelled", "bg-surface-2 text-muted"],
};
const ACTIVE = ["CREATED", "PLANNING", "READY", "RUNNING", "RETRYING", "WAITING_FOR_APPROVAL", "NEEDS_CLARIFICATION", "WAITING_FOR_EXTERNAL_RESPONSE"];
const UNSUPPORTED_ACTIONS: Record<string, string> = { book: "booking", buy: "buying", pay: "paying", send: "sending messages", apply: "submitting applications", negotiate: "live negotiation" };

function StateBadge({ state }: { state: string }) {
  const [text, cls] = STATE[state] ?? [state, "bg-surface-2 text-muted"];
  return (
    <span className={`inline-flex h-6 shrink-0 items-center gap-1 rounded-full px-2 text-[11.5px] font-medium ${cls}`}>
      {["RUNNING", "PLANNING", "RETRYING"].includes(state) && <Loader2 size={11} className="animate-spin" aria-hidden />}
      {text}
    </span>
  );
}

function Chip({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return <span className="inline-flex items-center gap-1 rounded-full border border-line bg-surface px-2 py-0.5 text-[11.5px]">{icon}{children}</span>;
}

function Understanding({ p }: { p: TaskPreview }) {
  const h = p.hints;
  const [ne, en] = CATEGORY[h.category] ?? [h.category, h.category];
  const unsupported = h.sensitive_actions.map((a) => UNSUPPORTED_ACTIONS[a]).filter(Boolean);
  return (
    <div className="mt-2 space-y-2 rounded-xl bg-surface-2 p-3 text-left text-[12px]" aria-live="polite">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="font-medium text-muted">Understood:</span>
        <Chip icon={<Sparkles size={11} className="text-accent-fg" aria-hidden />}><span className="deva">{ne}</span> · {en}</Chip>
        {h.dates.map((d) => <Chip key={d.text} icon={<Calendar size={11} aria-hidden />}>{d.text} → {d.date}</Chip>)}
        {h.times.map((t) => <Chip key={t} icon={<Clock size={11} aria-hidden />}>{t}</Chip>)}
        {h.amounts.map((a) => <Chip key={a.text} icon={<Coins size={11} aria-hidden />}>{a.currency} {a.amount.toLocaleString()}</Chip>)}
        {h.locations.map((l) => <Chip key={l} icon={<MapPin size={11} aria-hidden />}>{l}</Chip>)}
        {h.preferences.map((x) => <Chip key={x} icon={<Check size={11} aria-hidden />}>{x.replace("_", " ")}</Chip>)}
      </div>
      {p.workflow && <p><span className="font-medium">Workflow:</span> {p.workflow}</p>}
      {unsupported.length > 0 && (
        <p className="flex items-start gap-1.5 text-warn-text">
          <AlertTriangle size={13} className="mt-0.5 shrink-0" aria-hidden />
          DigitsNepal can prepare this, but cannot do the {unsupported.join(", ")} itself; that stays with you.
        </p>
      )}
    </div>
  );
}

function StepIcon({ s }: { s: PlanStep["status"] }) {
  if (s === "done") return <CheckCircle2 size={16} className="text-ok" aria-label="Done" />;
  if (s === "running") return <Loader2 size={16} className="animate-spin text-accent-fg" aria-label="In progress" />;
  if (s === "failed") return <XCircle size={16} className="text-error-text" aria-label="Failed" />;
  if (s === "skipped") return <CircleDashed size={16} className="text-muted" aria-label="Skipped" />;
  return <Circle size={16} className="text-muted" aria-label="Not started" />;
}

function ago(ms: number) {
  const s = Math.max(1, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return new Date(ms).toLocaleDateString();
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-2">
      <h3 className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">{title}</h3>
      {children}
    </section>
  );
}

function Detail({ d, pending, onControl, onOpenChat, onDecide }: {
  d: TaskDetail; pending: PendingItem[]; onControl: (a: "pause" | "resume" | "cancel" | "retry") => void; onOpenChat: () => void; onDecide: (id: string, x: "once" | "deny") => void;
}) {
  const t = d.task;
  const steps = t.plan.steps ?? [];
  const done = steps.filter((s) => s.status === "done").length;
  const active = ACTIVE.includes(t.state);
  return (
    <div className="space-y-5">
      <div className="space-y-2">
        <div className="flex items-start justify-between gap-3">
          <p data-selectable className="text-[15px] font-semibold leading-snug">{t.request}</p>
          <StateBadge state={t.state} />
        </div>
        {t.state_reason && <p className="text-[12.5px] text-muted">{t.state_reason}</p>}
        <div className="flex flex-wrap gap-2">
          {active && !t.paused && <Button onClick={() => onControl("pause")}><Pause size={13} aria-hidden /> Pause</Button>}
          {(t.state === "BLOCKED" || t.state === "NEEDS_CLARIFICATION") && <Button variant="primary" onClick={() => onControl("resume")}><Play size={13} aria-hidden /> Resume</Button>}
          {t.state === "FAILED" && <Button variant="primary" onClick={() => onControl("retry")}><RotateCcw size={13} aria-hidden /> Retry</Button>}
          {!["COMPLETED", "CANCELLED"].includes(t.state) && <Button variant="danger" onClick={() => onControl("cancel")}><X size={13} aria-hidden /> Cancel task</Button>}
          <Button onClick={onOpenChat}><MessageSquare size={13} aria-hidden /> Open conversation</Button>
        </div>
      </div>

      {pending.length > 0 && (
        <Section title="Needs your approval">
          {pending.map((p) => (
            <div key={p.id} className="rounded-xl border border-accent bg-surface p-3 shadow-card">
              <p className="text-[13px] font-semibold">{label(p.tool)}</p>
              <p className="mt-0.5 whitespace-pre-line text-[12px] text-muted">{p.reason}</p>
              <pre data-selectable className="mt-2 max-h-48 overflow-auto rounded-lg bg-surface-2 p-2 text-[11px]">{JSON.stringify(p.preview, null, 2)}</pre>
              <div className="mt-2 flex gap-2">
                {p.live ? (
                  <>
                    <Button variant="primary" onClick={() => onDecide(p.id, "once")}><Check size={13} aria-hidden /> Allow once</Button>
                    <Button variant="danger" onClick={() => onDecide(p.id, "deny")}><Ban size={13} aria-hidden /> Deny</Button>
                  </>
                ) : <Button onClick={() => onControl("resume")}>Resume to answer</Button>}
              </div>
            </div>
          ))}
        </Section>
      )}

      <Section title={steps.length ? `Plan · ${done} of ${steps.length} done` : "Plan"}>
        {t.plan.summary && <p className="text-[13px]">{t.plan.summary}</p>}
        {(t.plan.warnings ?? []).map((w) => (
          <p key={w} className="flex items-start gap-1.5 rounded-lg bg-warn-bg px-2.5 py-1.5 text-[12px] text-warn-text"><AlertTriangle size={13} className="mt-0.5 shrink-0" aria-hidden /> {w}</p>
        ))}
        {steps.length === 0 ? (
          <p className="text-[12.5px] text-muted">{active ? "Sahayogi is making a plan…" : "No plan was recorded."}</p>
        ) : (
          <ol className="space-y-1.5">
            {steps.map((s, i) => (
              <li key={i} className="flex items-start gap-2.5">
                <span className="mt-0.5"><StepIcon s={s.status} /></span>
                <span className="min-w-0">
                  <span className={`block text-[13px] ${s.status === "done" ? "text-muted line-through decoration-1" : ""}`}>{s.title}</span>
                  {(s.tool || s.needs_approval) && <span className="block text-[11px] text-muted">{s.tool && label(s.tool)}{s.needs_approval && " · asks you first"}</span>}
                  {s.note && <span className="block text-[11.5px] text-muted">{s.note}</span>}
                </span>
              </li>
            ))}
          </ol>
        )}
      </Section>

      {t.result.summary && (
        <Section title="Result">
          <div className={`rounded-xl border p-3.5 ${t.result.verified ? "border-ok/40 bg-ok-bg" : "border-warn-text/40 bg-warn-bg"}`}>
            <p className="flex items-center gap-1.5 text-[12px] font-semibold">
              {t.result.verified ? <><ShieldCheck size={14} className="text-ok" aria-hidden /> Verified by DigitsNepal</> : <><AlertTriangle size={14} className="text-warn-text" aria-hidden /> Not everything could be verified</>}
            </p>
            <p data-selectable className="mt-1.5 whitespace-pre-wrap text-[13px] leading-relaxed">{t.result.summary}</p>
          </div>
          {(t.result.actions ?? []).length > 0 && (
            <ul className="space-y-1">
              {t.result.actions!.map((a, i) => (
                <li key={i} className="flex items-start gap-2 text-[12px]">
                  {a.verified ? <Check size={14} className="mt-0.5 shrink-0 text-ok" aria-label="Verified" /> : <XCircle size={14} className="mt-0.5 shrink-0 text-error-text" aria-label="Not verified" />}
                  <span><span className="font-medium">{label(a.tool)}</span> <span className="font-mono text-[11px] text-muted">{a.target}</span><span className="block text-[11px] text-muted">{a.why}</span></span>
                </li>
              ))}
            </ul>
          )}
          {(t.result.next_steps ?? []).length > 0 && (
            <div>
              <p className="text-[12px] font-medium">What you still need to do</p>
              <ul className="mt-1 list-disc space-y-0.5 pl-5 text-[12.5px]">{t.result.next_steps!.map((n) => <li key={n} data-selectable>{n}</li>)}</ul>
            </div>
          )}
        </Section>
      )}

      {t.evidence.length > 0 && (
        <Section title={`Sources (${t.evidence.length})`}>
          <ul className="space-y-1">
            {t.evidence.map((e, i) => (
              <li key={i} className="flex items-start gap-2 text-[12px]">
                <ExternalLink size={12} className="mt-0.5 shrink-0 text-muted" aria-hidden />
                <span className="min-w-0"><span className="block">{e.title}</span><span data-selectable className="block truncate font-mono text-[11px] text-muted" title={e.source}>{e.source}</span></span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      <Section title="History">
        <ol className="space-y-1 border-l border-line pl-3">
          {d.events.map((e, i) => (
            <li key={i} className="text-[11.5px]">
              <span className="text-muted">{new Date(e.ts).toLocaleTimeString()} · </span>
              {e.kind === "state" ? <>{STATE[e.from]?.[0] ?? e.from} → <span className="font-medium">{STATE[e.to]?.[0] ?? e.to}</span>{typeof e.detail === "string" && e.detail ? <span className="text-muted"> — {e.detail}</span> : null}</>
                : e.kind === "step" ? <>Step {(e.detail as { step?: number }).step}: {(e.detail as { status?: string }).status}</>
                : e.kind === "plan" ? <>Plan recorded</>
                : e.kind === "result" ? <>Result reported{(e.detail as { verified?: boolean }).verified ? " (verified)" : " (not verified)"}</>
                : e.kind}
            </li>
          ))}
        </ol>
        <p className="text-[11px] text-muted">{(d.input_tokens + d.output_tokens).toLocaleString()} tokens · {d.interventions} decision{d.interventions === 1 ? "" : "s"} by you · started {ago(t.created_at)}</p>
      </Section>
    </div>
  );
}

export function TasksPage({ onOpenChat }: { onOpenChat: (session: string) => void }) {
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<TaskPreview | null>(null);
  const [tasks, setTasks] = useState<TaskRow[]>([]);
  const [metrics, setMetrics] = useState<TaskMetrics | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<TaskDetail | null>(null);
  const [pending, setPending] = useState<PendingItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const selRef = useRef<string | null>(null);
  selRef.current = selected;

  const refresh = useCallback(async () => {
    try {
      setTasks(await cw.tasks());
      setMetrics(await cw.taskMetrics());
      setPending(await cw.pending());
      if (selRef.current) setDetail(await cw.task(selRef.current));
    } catch (e) {
      setError(toAppError(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    let timer: ReturnType<typeof setTimeout> | null = null;
    const un = listen<CoworkerEvent>("coworker:event", () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => void refresh(), 250);
    });
    return () => void un.then((f) => f());
  }, [refresh]);

  useEffect(() => {
    if (selected) void cw.task(selected).then(setDetail).catch((e) => setError(toAppError(e)));
    else setDetail(null);
  }, [selected]);

  // Offline understanding as the user types (no model call).
  useEffect(() => {
    if (!text.trim()) return setPreview(null);
    const t = setTimeout(() => void cw.taskPreview(text).then(setPreview).catch(() => setPreview(null)), 200);
    return () => clearTimeout(t);
  }, [text]);

  const start = async () => {
    if (!text.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const t = await cw.createTask(text);
      setText("");
      setSelected(t.id);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setBusy(false);
    }
  };

  const control = async (a: "pause" | "resume" | "cancel" | "retry") => {
    if (!detail) return;
    setError(null);
    try {
      await cw.taskControl(detail.task.id, a);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  const pct = (x: number | null) => (x == null ? "—" : `${Math.round(x * 100)}%`);

  return (
    <div className="h-full overflow-auto bg-bg">
      <div className="mx-auto max-w-6xl space-y-6 p-6">
        <section className="mx-auto max-w-3xl pt-2 text-center">
          <h1 className="deva text-[24px] font-semibold tracking-tight">आज तपाईंलाई के काममा सहयोग चाहिन्छ?</h1>
          <p className="mt-1 text-[13px] text-muted">What would you like me to do today? Write in नेपाली, Romanized Nepali or English.</p>
          <form className="mt-4" onSubmit={(e) => { e.preventDefault(); void start(); }}>
            <div className="flex items-end gap-2 rounded-2xl border border-line bg-surface px-3 py-2 text-left shadow-card focus-within:border-accent">
              <label htmlFor="task-input" className="sr-only">Describe the task</label>
              <textarea
                id="task-input"
                rows={2}
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void start(); } }}
                placeholder="e.g. Malai bholi ghar safa garne service chahiyo, budget 3 hajar"
                className="max-h-48 min-h-[52px] flex-1 resize-none bg-transparent py-1.5 text-[15px] leading-relaxed outline-none placeholder:text-muted"
                spellCheck={false}
              />
              <button type="submit" aria-label="Start task" disabled={!text.trim() || busy} className="flex h-10 shrink-0 items-center gap-1.5 rounded-full bg-accent px-4 text-[13px] font-medium text-on-accent transition-colors hover:bg-accent-hover disabled:bg-surface-2 disabled:text-muted">
                {busy ? <Loader2 size={15} className="animate-spin" aria-hidden /> : <ArrowUp size={15} strokeWidth={2.4} aria-hidden />} Start task
              </button>
            </div>
            {preview && <Understanding p={preview} />}
          </form>
          {!text && (
            <div className="mt-4 grid gap-2 text-left sm:grid-cols-3">
              {EXAMPLES.map(({ icon: Icon, title, text: t }) => (
                <button key={title} onClick={() => setText(t)} className="rounded-xl border border-line bg-surface p-3 text-left shadow-card transition-colors hover:border-accent hover:bg-accent-soft">
                  <span className="flex items-center gap-1.5 text-[12px] font-semibold"><Icon size={14} className="text-accent-fg" aria-hidden /> {title}</span>
                  <span className="mt-1 block text-[12px] leading-snug text-muted">{t}</span>
                </button>
              ))}
            </div>
          )}
          <p className="mt-3 text-[11.5px] text-muted">Sahayogi plans, researches and prepares. It never books, pays, sends or submits for you; every result is checked against what actually happened.</p>
        </section>

        <ErrorBanner error={error} title="Something went wrong" note={null} />

        {metrics && metrics.total > 0 && (
          <dl className="grid grid-cols-2 gap-2 sm:grid-cols-5">
            {[
              ["Tasks", String(metrics.total)],
              ["Completed", pct(metrics.completion_rate)],
              ["Verified results", pct(metrics.verified_rate)],
              ["Your decisions / task", metrics.interventions_per_task == null ? "—" : metrics.interventions_per_task.toFixed(1)],
              ["Tokens / task", metrics.tokens_per_task == null ? "—" : Math.round(metrics.tokens_per_task).toLocaleString()],
            ].map(([k, v]) => (
              <div key={k} className="rounded-xl border border-line bg-surface p-3 shadow-card">
                <dt className="text-[11px] text-muted">{k}</dt>
                <dd className="mt-0.5 text-[18px] font-semibold tabular-nums">{v}</dd>
              </div>
            ))}
          </dl>
        )}

        {tasks.length > 0 && (
          <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
            <section aria-labelledby="my-tasks-h">
              <h2 id="my-tasks-h" className="mb-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">My tasks</h2>
              <ul className="space-y-1.5">
                {tasks.map((t) => {
                  const steps = t.plan.steps ?? [];
                  const waiting = pending.some((p) => p.session === t.session && p.kind === "approval");
                  return (
                    <li key={t.id}>
                      <button
                        onClick={() => setSelected(t.id)}
                        aria-current={t.id === selected ? "true" : undefined}
                        className={`w-full rounded-xl border p-3 text-left shadow-card transition-colors ${t.id === selected ? "border-accent bg-accent-soft" : "border-line bg-surface hover:border-accent"}`}
                      >
                        <span className="flex items-start justify-between gap-2">
                          <span className="line-clamp-2 text-[13px] font-medium">{t.request}</span>
                          <StateBadge state={t.state} />
                        </span>
                        <span className="mt-1 flex items-center gap-2 text-[11px] text-muted">
                          <span className="deva">{CATEGORY[t.intent?.category]?.[0] ?? ""}</span>
                          {steps.length > 0 && <span>· {steps.filter((s) => s.status === "done").length}/{steps.length} steps</span>}
                          {waiting && <span className="font-medium text-warn-text">· approval waiting</span>}
                          <span className="ml-auto">{ago(t.updated_at)}</span>
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            </section>
            <section aria-label="Task details" className="rounded-2xl border border-line bg-surface p-5 shadow-card">
              {detail ? (
                <Detail
                  d={detail}
                  pending={pending.filter((p) => p.session === detail.task.session && p.kind === "approval")}
                  onControl={(a) => void control(a)}
                  onOpenChat={() => onOpenChat(detail.task.session)}
                  onDecide={(id, x) => void cw.decide(id, x).then(refresh)}
                />
              ) : (
                <p className="py-10 text-center text-[13px] text-muted">Choose a task to see its plan, progress and result.</p>
              )}
            </section>
          </div>
        )}
      </div>
    </div>
  );
}
