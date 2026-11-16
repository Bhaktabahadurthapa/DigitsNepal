import { useEffect, useState } from "react";
import { Button, ErrorBanner, Notice } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type Activity } from "./api";
import { label } from "./CoworkerPage";

const tone: Record<string, string> = { allow: "text-emerald-600", deny: "text-red-600", ask: "text-amber-600", once: "text-emerald-600", session: "text-emerald-600" };

export function ActivityPage() {
  const [a, setA] = useState<Activity | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const load = () => cw.activity(500).then(setA).catch((e) => setError(toAppError(e)));
  useEffect(() => void load(), []);
  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="mb-3 flex items-center justify-between">
        <div>
          <h1 className="text-[17px] font-semibold">Activity</h1>
          <p className="text-[12.5px] text-muted">Every tool the coworker used, who approved it and why. File contents are never stored here, only fingerprints.</p>
        </div>
        <Button onClick={() => void load()}>Refresh</Button>
      </div>
      <ErrorBanner error={error} />
      {a && (a.intact ? (
        <Notice tone="info">Integrity check passed: the log has not been edited ({a.entries.length} recent entries).</Notice>
      ) : (
        <Notice>Integrity check failed at entry #{a.broken_at}. The log was changed outside DigitsNepal.</Notice>
      ))}
      <table className="mt-3 w-full text-left text-[12.5px]">
        <thead className="text-muted">
          <tr><th className="py-1">Time</th><th>Who</th><th>What</th><th>Tool</th><th>Decision</th><th>Why</th></tr>
        </thead>
        <tbody>
          {a?.entries.map((e) => (
            <tr key={e.seq} className="border-t border-line align-top">
              <td className="py-1 pr-2 whitespace-nowrap text-muted">{new Date(e.ts).toLocaleString()}</td>
              <td className="pr-2">{e.actor}</td>
              <td className="pr-2">{e.kind.replace("_", " ")}</td>
              <td className="pr-2">{e.tool ? label(e.tool) : "—"}</td>
              <td className={`pr-2 font-medium ${tone[e.decision] ?? ""}`}>{e.decision}</td>
              <td className="max-w-[360px] break-words text-muted">{e.detail}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
