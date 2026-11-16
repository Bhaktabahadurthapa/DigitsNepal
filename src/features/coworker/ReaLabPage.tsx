import { useEffect, useState } from "react";
import { Button, ErrorBanner, Notice, Panel } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type Investigation, type Readiness, type Target } from "./api";

const TYPES: { id: Investigation; label: string; hint: string; kind: "origin" | "path" }[] = [
  { id: "website", label: "Website", hint: "How a site's features, pages and APIs work", kind: "origin" },
  { id: "javascript_app", label: "JavaScript / Electron app", hint: "An app folder or .asar you own", kind: "path" },
  { id: "desktop_app", label: "Desktop app or disk image", hint: "A .app, .dmg or package you are authorized to inspect", kind: "path" },
];

export function ReaLabPage({ onStart }: { onStart: (investigation: Investigation, prompt: string) => void }) {
  const [ready, setReady] = useState<Readiness | null>(null);
  const [targets, setTargets] = useState<Target[]>([]);
  const [type, setType] = useState<Investigation>("website");
  const [value, setValue] = useState("");
  const [attest, setAttest] = useState("");
  const [question, setQuestion] = useState("Explain how the main features work, with evidence.");
  const [error, setError] = useState<AppError | null>(null);

  const load = () => {
    void cw.reaReadiness().then(setReady);
    void cw.reaTargets().then(setTargets).catch((e) => setError(toAppError(e)));
  };
  useEffect(load, []);
  const kind = TYPES.find((t) => t.id === type)!.kind;

  const start = async () => {
    setError(null);
    try {
      const t = targets.find((x) => x.value === value.trim()) ?? (await cw.reaAddTarget(kind, value, attest));
      setTargets((all) => (all.some((x) => x.id === t.id) ? all : [...all, t]));
      onStart(type, `Target: ${t.value}\n${question}`);
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <div className="mx-auto max-w-3xl space-y-4 p-6">
      <div>
        <h1 className="text-[17px] font-semibold">REA Lab</h1>
        <p className="text-[12.5px] text-muted">Investigate software you own or are authorized to inspect, powered by REA (rea.tools). Findings come with evidence and stated limitations.</p>
      </div>
      <ErrorBanner error={error} />
      <Panel title="REA status" actions={<Button onClick={load}>Check again</Button>}>
        {!ready ? <p className="text-[13px] text-muted">Checking…</p> : ready.ready ? (
          <p className="text-[13px]">Ready: REA {ready.rea_version} with Node {ready.node_version}.</p>
        ) : (
          <ul className="list-disc space-y-1 pl-5 text-[13px]">{ready.problems.map((p) => <li key={p}>{p}</li>)}</ul>
        )}
      </Panel>
      <Panel title="New investigation">
        <fieldset className="grid gap-2 sm:grid-cols-3">
          <legend className="sr-only">Investigation type</legend>
          {TYPES.map((t) => (
            <label key={t.id} className={`cursor-pointer rounded-md border p-2 text-[12.5px] ${type === t.id ? "border-accent bg-accent-soft" : "border-line"}`}>
              <input type="radio" name="inv" className="sr-only" checked={type === t.id} onChange={() => setType(t.id)} />
              <b>{t.label}</b><br /><span className="text-muted">{t.hint}</span>
            </label>
          ))}
        </fieldset>
        <label className="mt-3 block text-[12.5px] font-medium" htmlFor="rea-target">{kind === "origin" ? "Website address" : "Full path"}</label>
        <input id="rea-target" value={value} onChange={(e) => setValue(e.target.value)} placeholder={kind === "origin" ? "https://digitsnepal.com" : "/Applications/MyApp.app"} className="mt-1 w-full rounded border border-line bg-surface px-2 py-1.5 text-[13px]" />
        <label className="mt-3 block text-[12.5px] font-medium" htmlFor="rea-attest">Authorization</label>
        <input id="rea-attest" value={attest} onChange={(e) => setAttest(e.target.value)} placeholder="e.g. I own this website" className="mt-1 w-full rounded border border-line bg-surface px-2 py-1.5 text-[13px]" />
        <label className="mt-3 block text-[12.5px] font-medium" htmlFor="rea-q">Question</label>
        <textarea id="rea-q" rows={2} value={question} onChange={(e) => setQuestion(e.target.value)} className="mt-1 w-full rounded border border-line bg-surface px-2 py-1.5 text-[13px]" />
        <div className="mt-3 flex justify-end"><Button variant="primary" disabled={!ready?.ready || !value.trim()} onClick={() => void start()}>Start investigation</Button></div>
      </Panel>
      <Panel title="Authorized targets">
        {targets.length === 0 ? <p className="text-[13px] text-muted">None yet.</p> : (
          <ul className="space-y-1 text-[13px]">
            {targets.map((t) => (
              <li key={t.id} className="flex items-center justify-between gap-2">
                <span><b>{t.value}</b> <span className="text-muted">— {t.attestation}</span></span>
                <Button onClick={() => void cw.reaRevokeTarget(t.id).then(load)}>Remove</Button>
              </li>
            ))}
          </ul>
        )}
        <Notice tone="info">Steps that launch, drive or instrument a target always ask you first.</Notice>
      </Panel>
    </div>
  );
}
