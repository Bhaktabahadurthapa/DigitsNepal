import { AppWindow, CheckCircle2, Code2, Globe, Loader2, Play, RefreshCw, ScanSearch, ShieldCheck, Trash2, TriangleAlert } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button, ErrorBanner } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type Investigation, type Readiness, type Target } from "./api";

const TYPES: { id: Investigation; label: string; hint: string; kind: "origin" | "path"; icon: typeof Globe }[] = [
  { id: "website", label: "Website", hint: "Pages, features and APIs of a site", kind: "origin", icon: Globe },
  { id: "javascript_app", label: "JavaScript / Electron app", hint: "An app folder or .asar you own", kind: "path", icon: Code2 },
  { id: "desktop_app", label: "Desktop app", hint: "A .app, .dmg or package", kind: "path", icon: AppWindow },
];

function Step({ n, title, children }: { n: number; title: string; children: ReactNode }) {
  return (
    <section className="rounded-xl border border-line bg-surface p-5 shadow-card">
      <h2 className="mb-3 flex items-center gap-2.5 text-[14px] font-semibold">
        <span className="flex h-6 w-6 items-center justify-center rounded-full bg-accent-soft text-[12px] text-accent-fg">{n}</span>
        {title}
      </h2>
      {children}
    </section>
  );
}

const inputCls = "h-9 w-full rounded-lg border border-line bg-surface px-3 text-[13px] outline-none transition-colors placeholder:text-muted focus:border-accent";

export function ReaLabPage({ onStart }: { onStart: (investigation: Investigation, prompt: string) => void }) {
  const [ready, setReady] = useState<Readiness | null>(null);
  const [targets, setTargets] = useState<Target[]>([]);
  const [type, setType] = useState<Investigation>("website");
  const [value, setValue] = useState("");
  const [attest, setAttest] = useState("");
  const [question, setQuestion] = useState("Explain how the main features work, with evidence.");
  const [error, setError] = useState<AppError | null>(null);

  const load = () => {
    setReady(null);
    void cw.reaReadiness().then(setReady);
    void cw.reaTargets().then(setTargets).catch((e) => setError(toAppError(e)));
  };
  useEffect(load, []);
  const kind = TYPES.find((t) => t.id === type)!.kind;
  const known = targets.find((x) => x.value === value.trim());

  const start = async () => {
    setError(null);
    try {
      const t = known ?? (await cw.reaAddTarget(kind, value, attest));
      setTargets((all) => (all.some((x) => x.id === t.id) ? all : [...all, t]));
      onStart(type, `Target: ${t.value}\n${question}`);
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <div className="h-full overflow-auto bg-bg">
      <div className="mx-auto max-w-3xl space-y-4 p-6">
        <div className="flex items-start gap-3">
          <div className="brand-gradient flex h-11 w-11 items-center justify-center rounded-xl text-white shadow-card" aria-hidden><ScanSearch size={22} /></div>
          <div>
            <h1 className="text-[20px] font-semibold tracking-tight">REA Lab</h1>
            <p className="mt-0.5 text-[13px] text-muted">Understand how software you own or are authorized to inspect works. Every finding comes with evidence and its limits.</p>
          </div>
        </div>
        <ErrorBanner error={error} title="Could not start the investigation" note={null} />

        <section className={`flex items-start gap-3 rounded-xl border p-4 shadow-card ${ready?.ready ? "border-line bg-surface" : "border-line bg-surface"}`}>
          <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${!ready ? "bg-surface-2 text-muted" : ready.ready ? "bg-ok-bg text-ok" : "bg-warn-bg text-warn-text"}`}>
            {!ready ? <Loader2 size={18} className="animate-spin" aria-hidden /> : ready.ready ? <CheckCircle2 size={18} aria-hidden /> : <TriangleAlert size={18} aria-hidden />}
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-[13.5px] font-semibold">{!ready ? "Checking REA…" : ready.ready ? `REA ${ready.rea_version} is ready` : "REA needs attention"}</p>
            {ready?.ready && <p className="text-[12.5px] text-muted">Node {ready.node_version}</p>}
            {ready && !ready.ready && (
              <ul className="mt-1 space-y-1 text-[12.5px] text-muted">
                {ready.problems.map((p) => <li key={p} data-selectable>{p}</li>)}
              </ul>
            )}
          </div>
          <Button onClick={load}><RefreshCw size={14} aria-hidden /> Check again</Button>
        </section>

        <Step n={1} title="What do you want to investigate?">
          <fieldset className="grid gap-2 sm:grid-cols-3">
            <legend className="sr-only">Investigation type</legend>
            {TYPES.map(({ id, label, hint, icon: Icon }) => (
              <label key={id} className={`flex cursor-pointer flex-col gap-1.5 rounded-xl border p-3 transition-colors ${type === id ? "border-accent bg-accent-soft" : "border-line hover:bg-surface-2"}`}>
                <input type="radio" name="inv" className="sr-only" checked={type === id} onChange={() => setType(id)} />
                <Icon size={18} className={type === id ? "text-accent-fg" : "text-muted"} aria-hidden />
                <span className="text-[13px] font-medium">{label}</span>
                <span className="text-[11.5px] text-muted">{hint}</span>
              </label>
            ))}
          </fieldset>
        </Step>

        <Step n={2} title="Target and authorization">
          <label className="block text-[12.5px] font-medium" htmlFor="rea-target">{kind === "origin" ? "Website address" : "Full path on this Mac"}</label>
          <input id="rea-target" value={value} onChange={(e) => setValue(e.target.value)} placeholder={kind === "origin" ? "https://digitsnepal.com" : "/Applications/MyApp.app"} className={`mt-1.5 ${inputCls}`} />
          {!known && (
            <>
              <label className="mt-3 block text-[12.5px] font-medium" htmlFor="rea-attest">Why you may inspect it</label>
              <input id="rea-attest" value={attest} onChange={(e) => setAttest(e.target.value)} placeholder="e.g. I own this website" className={`mt-1.5 ${inputCls}`} />
              <p className="mt-1.5 text-[11.5px] text-muted">Only investigate software you own or have permission to inspect.</p>
            </>
          )}
        </Step>

        <Step n={3} title="Your question">
          <label htmlFor="rea-q" className="sr-only">Question</label>
          <textarea id="rea-q" rows={2} value={question} onChange={(e) => setQuestion(e.target.value)} className="w-full rounded-lg border border-line bg-surface px-3 py-2 text-[13px] outline-none transition-colors focus:border-accent" />
          <div className="mt-3 flex items-center justify-between gap-3">
            <p className="flex items-center gap-1.5 text-[11.5px] text-muted"><ShieldCheck size={13} aria-hidden /> Steps that run or drive the target always ask you first.</p>
            <Button variant="primary" disabled={!ready?.ready || !value.trim() || (!known && attest.trim().length < 8)} onClick={() => void start()}>
              <Play size={14} aria-hidden /> Start investigation
            </Button>
          </div>
        </Step>

        <section className="rounded-xl border border-line bg-surface p-5 shadow-card">
          <h2 className="mb-2 text-[14px] font-semibold">Authorized targets</h2>
          {targets.length === 0 ? (
            <p className="text-[12.5px] text-muted">None yet. Targets you authorize appear here.</p>
          ) : (
            <ul className="divide-y divide-line">
              {targets.map((t) => (
                <li key={t.id} className="flex items-center gap-3 py-2">
                  {t.kind === "origin" ? <Globe size={15} className="text-muted" aria-hidden /> : <AppWindow size={15} className="text-muted" aria-hidden />}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13px] font-medium">{t.value}</p>
                    <p className="truncate text-[11.5px] text-muted">{t.attestation}</p>
                  </div>
                  <button aria-label={`Remove ${t.value}`} className="rounded-lg p-2 text-muted transition-colors hover:bg-error-bg hover:text-error-text" onClick={() => void cw.reaRevokeTarget(t.id).then(load)}>
                    <Trash2 size={14} aria-hidden />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
