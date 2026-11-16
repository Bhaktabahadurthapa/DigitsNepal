import { Ban, Check, CircleHelp, RefreshCw, ShieldAlert, ShieldCheck, User, Bot, Cog } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, ErrorBanner } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type Activity, type AuditEntry } from "./api";
import { label } from "./CoworkerPage";

type Filter = "all" | "approvals" | "blocked" | "access";
const FILTERS: { id: Filter; label: string; test: (e: AuditEntry) => boolean }[] = [
  { id: "all", label: "All", test: () => true },
  { id: "approvals", label: "Approvals", test: (e) => e.kind === "approval" || e.decision === "ask" },
  { id: "blocked", label: "Blocked", test: (e) => e.decision === "deny" },
  { id: "access", label: "Access changes", test: (e) => e.kind === "grant" || e.kind === "authorize_target" },
];

function Decision({ d }: { d: string }) {
  const map: Record<string, [string, string, typeof Check]> = {
    allow: ["Allowed", "bg-ok-bg text-ok", Check],
    once: ["Approved once", "bg-ok-bg text-ok", Check],
    session: ["Approved for chat", "bg-ok-bg text-ok", Check],
    granted: ["Granted", "bg-ok-bg text-ok", Check],
    authorized: ["Authorized", "bg-ok-bg text-ok", Check],
    deny: ["Blocked", "bg-error-bg text-error-text", Ban],
    revoked: ["Revoked", "bg-surface-2 text-muted", Ban],
    ask: ["Asked you", "bg-warn-bg text-warn-text", CircleHelp],
  };
  const [text, cls, Icon] = map[d] ?? [d, "bg-surface-2 text-muted", CircleHelp];
  return (
    <span className={`inline-flex h-6 items-center gap-1 rounded-full px-2 text-[11.5px] font-medium ${cls}`}>
      <Icon size={12} aria-hidden /> {text}
    </span>
  );
}

const ACTOR: Record<string, [string, typeof User]> = { user: ["You", User], policy: ["Policy", ShieldCheck], agent: ["Sahayogi", Bot], system: ["System", Cog] };

export function ActivityPage() {
  const [a, setA] = useState<Activity | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<AppError | null>(null);
  const load = () => cw.activity(500).then(setA).catch((e) => setError(toAppError(e)));
  useEffect(() => void load(), []);
  const rows = useMemo(() => (a?.entries ?? []).filter(FILTERS.find((f) => f.id === filter)!.test), [a, filter]);

  return (
    <div className="h-full overflow-auto bg-bg">
      <div className="mx-auto max-w-5xl space-y-4 p-6">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-[20px] font-semibold tracking-tight">Activity</h1>
            <p className="mt-0.5 text-[13px] text-muted">Everything Sahayogi did, who approved it and why. File contents are never stored, only fingerprints.</p>
          </div>
          <Button onClick={() => void load()}><RefreshCw size={14} aria-hidden /> Refresh</Button>
        </div>
        <ErrorBanner error={error} title="Could not load activity" note={null} />

        {a && (
          <div className={`flex items-center gap-3 rounded-xl border p-4 shadow-card ${a.intact ? "border-line bg-surface" : "border-warn-text/30 bg-warn-bg"}`}>
            <span className={`flex h-10 w-10 items-center justify-center rounded-xl ${a.intact ? "bg-ok-bg text-ok" : "bg-surface text-warn-text"}`}>
              {a.intact ? <ShieldCheck size={20} aria-hidden /> : <ShieldAlert size={20} aria-hidden />}
            </span>
            <div>
              <p className="text-[13.5px] font-semibold">{a.intact ? "Integrity verified" : `Integrity check failed at entry #${a.broken_at}`}</p>
              <p className="text-[12.5px] text-muted">{a.intact ? `The log has not been edited. ${a.entries.length} recent entries.` : "The log was changed outside DigitsNepal."}</p>
            </div>
          </div>
        )}

        <div className="flex gap-1.5" role="tablist" aria-label="Filter activity">
          {FILTERS.map((f) => (
            <button key={f.id} role="tab" aria-selected={filter === f.id} onClick={() => setFilter(f.id)} className={`h-8 rounded-full px-3 text-[12.5px] font-medium transition-colors ${filter === f.id ? "bg-ink text-bg" : "bg-surface text-muted shadow-card hover:text-ink"}`}>
              {f.label}
            </button>
          ))}
        </div>

        <div className="overflow-hidden rounded-xl border border-line bg-surface shadow-card">
          <table className="w-full text-left text-[12.5px]">
            <thead className="bg-surface-2 text-[11px] uppercase tracking-[0.06em] text-muted">
              <tr>
                <th className="px-4 py-2.5 font-semibold">When</th>
                <th className="px-2 py-2.5 font-semibold">Who</th>
                <th className="px-2 py-2.5 font-semibold">Action</th>
                <th className="px-2 py-2.5 font-semibold">Result</th>
                <th className="px-4 py-2.5 font-semibold">Reason</th>
              </tr>
            </thead>
            <tbody>
              {rows.length === 0 ? (
                <tr><td colSpan={5} className="px-4 py-10 text-center text-muted">Nothing here yet.</td></tr>
              ) : rows.map((e) => {
                const [who, Icon] = ACTOR[e.actor] ?? [e.actor, Cog];
                return (
                  <tr key={e.seq} className="border-t border-line align-top transition-colors hover:bg-surface-2">
                    <td className="whitespace-nowrap px-4 py-2.5 text-muted">{new Date(e.ts).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })}</td>
                    <td className="px-2 py-2.5"><span className="inline-flex items-center gap-1.5"><Icon size={13} className="text-muted" aria-hidden /> {who}</span></td>
                    <td className="px-2 py-2.5">{e.tool ? label(e.tool) : e.kind === "grant" ? "Folder access" : e.kind === "message" ? "Message" : e.kind.replaceAll("_", " ")}</td>
                    <td className="px-2 py-2.5"><Decision d={e.decision} /></td>
                    <td className="max-w-[340px] break-words px-4 py-2.5 text-muted" data-selectable>{e.detail || "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
