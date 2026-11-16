import { AlertTriangle, FolderGit2, Globe, Link2, Play, Plug, RefreshCw, ShieldCheck, Trash2, Workflow } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, ErrorBanner, Toggle } from "../../components/ui";
import { toAppError } from "../../providers/api";
import type { AppError } from "../../types/translation";
import { cw, type Automation, type McpServer } from "./api";
import { label } from "./CoworkerPage";

const BUILT_IN: { icon: typeof Globe; title: string; text: string }[] = [
  { icon: Globe, title: "Web search and pages", text: "Search the web and read public pages. Asks each time (or once per chat); private and local network addresses are always refused." },
  { icon: FolderGit2, title: "GitHub (public, read-only)", text: "Search repositories and issues and read a project's README or files. Nothing is posted." },
  { icon: Workflow, title: "Documents and research", text: "Cited source packs from your files and pages; Markdown, HTML or text documents saved only after you approve." },
];

function when(ms: number | null) {
  return ms ? new Date(ms).toLocaleString() : "never";
}

export function ToolsPage() {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [autos, setAutos] = useState<Automation[]>([]);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  const load = async () => {
    try {
      setServers(await cw.mcpServers());
      setAutos(await cw.automations());
    } catch (e) {
      setError(toAppError(e));
    }
  };
  useEffect(() => void load(), []);

  const run = async (key: string, f: () => Promise<unknown>) => {
    setBusy(key);
    setError(null);
    try {
      await f();
      await load();
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setBusy(null);
    }
  };

  const toggle = (s: McpServer, tool: string, on: boolean) =>
    run(`${s.name}.${tool}`, () => cw.mcpEnable(s.name, on ? [...s.enabled, tool] : s.enabled.filter((t) => t !== tool)));

  return (
    <div className="h-full overflow-auto bg-bg">
      <div className="mx-auto max-w-4xl space-y-6 p-6">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-[20px] font-semibold tracking-tight">Tools &amp; Apps</h1>
            <p className="mt-0.5 text-[13px] text-muted">What Sahayogi can use beyond your folders. Every action is still checked by policy and logged in Activity.</p>
          </div>
          <Button onClick={() => void load()}><RefreshCw size={14} aria-hidden /> Refresh</Button>
        </div>
        <ErrorBanner error={error} title="Something went wrong" note={null} />

        <section aria-labelledby="builtin-h">
          <h2 id="builtin-h" className="mb-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Built in</h2>
          <div className="grid gap-2 sm:grid-cols-3">
            {BUILT_IN.map(({ icon: Icon, title, text }) => (
              <div key={title} className="rounded-xl border border-line bg-surface p-3.5 shadow-card">
                <Icon size={16} className="text-accent-fg" aria-hidden />
                <p className="mt-2 text-[13px] font-semibold">{title}</p>
                <p className="mt-1 text-[12px] leading-relaxed text-muted">{text}</p>
              </div>
            ))}
          </div>
        </section>

        <section aria-labelledby="apps-h" className="space-y-3">
          <div>
            <h2 id="apps-h" className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Connected apps (MCP)</h2>
            <p className="mt-1 text-[12.5px] text-muted">Add a remote MCP server by its https address. Its tools start switched off; turn on only the ones you need. Only the general Sahayogi uses them, and every call asks you first.</p>
          </div>
          <form
            className="flex flex-wrap items-end gap-2 rounded-xl border border-line bg-surface p-3 shadow-card"
            onSubmit={(e) => { e.preventDefault(); void run("add", async () => { await cw.mcpAdd(name, url); setName(""); setUrl(""); }); }}
          >
            <label className="flex flex-col gap-1 text-[12px] font-medium">
              Name
              <input value={name} onChange={(e) => setName(e.target.value.toLowerCase())} placeholder="notion" maxLength={20} className="h-8 w-36 rounded-lg border border-line bg-surface px-2.5 text-[13px] font-normal outline-none focus:border-accent" />
            </label>
            <label className="flex min-w-0 flex-1 flex-col gap-1 text-[12px] font-medium">
              Server address
              <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://mcp.example.com/mcp" className="h-8 rounded-lg border border-line bg-surface px-2.5 text-[13px] font-normal outline-none focus:border-accent" />
            </label>
            <Button type="submit" variant="primary" disabled={!name.trim() || !url.trim() || busy === "add"}><Link2 size={14} aria-hidden /> {busy === "add" ? "Connecting…" : "Connect"}</Button>
            <p className="w-full text-[11.5px] text-muted">Only add servers you trust. Servers that need a login token, and local MCP programs, are not supported yet (local programs need DigitsNepal's sandbox).</p>
          </form>

          {servers.length === 0 ? (
            <p className="rounded-xl border border-dashed border-line p-4 text-center text-[12.5px] text-muted">No connected apps.</p>
          ) : (
            servers.map((s) => (
              <div key={s.name} className="rounded-xl border border-line bg-surface shadow-card">
                <div className="flex items-center gap-3 border-b border-line px-4 py-3">
                  <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent-soft text-accent-fg"><Plug size={15} aria-hidden /></span>
                  <div className="min-w-0 flex-1">
                    <p className="text-[13.5px] font-semibold">{s.name}</p>
                    <p className="truncate font-mono text-[11.5px] text-muted" title={s.url}>{s.url}</p>
                  </div>
                  <span className="text-[12px] text-muted">{s.enabled.length} of {s.tools.length} on</span>
                  <Button variant="danger" onClick={() => void run(`rm-${s.name}`, () => cw.mcpRemove(s.name))} disabled={busy !== null}><Trash2 size={13} aria-hidden /> Remove</Button>
                </div>
                <ul className="divide-y divide-line">
                  {s.tools.map((t) => (
                    <li key={t.name} className="px-4 py-2">
                      <Toggle
                        checked={s.enabled.includes(t.name)}
                        disabled={busy !== null}
                        onChange={(on) => void toggle(s, t.name, on)}
                        label={t.name}
                        description={
                          <span>
                            {t.destructive ? <span className="mr-1.5 inline-flex items-center gap-1 text-warn-text"><AlertTriangle size={11} aria-hidden /> Can change data · always asks</span>
                              : t.read_only ? <span className="mr-1.5 inline-flex items-center gap-1 text-ok"><ShieldCheck size={11} aria-hidden /> Read-only</span> : null}
                            {t.description}
                          </span>
                        }
                      />
                    </li>
                  ))}
                </ul>
              </div>
            ))
          )}
        </section>

        <section aria-labelledby="auto-h" className="space-y-3">
          <div>
            <h2 id="auto-h" className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">Automations</h2>
            <p className="mt-1 text-[12.5px] text-muted">Saved by Sahayogi when you ask (try the Swachalit specialist). To run one, ask in a chat, e.g. “run weekly-brief”. You approve every run with all its steps shown.</p>
          </div>
          {autos.length === 0 ? (
            <p className="rounded-xl border border-dashed border-line p-4 text-center text-[12.5px] text-muted">No automations yet.</p>
          ) : (
            autos.map((a) => (
              <div key={a.id} className="rounded-xl border border-line bg-surface p-4 shadow-card">
                <div className="flex items-start gap-3">
                  <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent-fg"><Workflow size={15} aria-hidden /></span>
                  <div className="min-w-0 flex-1">
                    <p className="font-mono text-[13px] font-semibold">{a.name}</p>
                    <p className="text-[12.5px] text-muted">{a.description}</p>
                    <ol className="mt-2 space-y-1">
                      {a.steps.map((st, i) => (
                        <li key={i} className="flex items-center gap-2 text-[12px]">
                          <span className="flex h-5 w-5 items-center justify-center rounded-full bg-surface-2 text-[10.5px] font-semibold">{i + 1}</span>
                          <span>{label(st.tool)}</span>
                          <span className="min-w-0 truncate font-mono text-[11px] text-muted">{String(st.args.path ?? st.args.url ?? st.args.query ?? st.args.repo ?? "")}</span>
                        </li>
                      ))}
                    </ol>
                    <p className="mt-2 flex items-center gap-1.5 text-[11.5px] text-muted"><Play size={11} aria-hidden /> Ran {a.runs} time{a.runs === 1 ? "" : "s"} · last {when(a.last_run_at ? a.last_run_at : null)}</p>
                  </div>
                  <Button variant="danger" onClick={() => void run(`del-${a.id}`, () => cw.deleteAutomation(a.id))} disabled={busy !== null}><Trash2 size={13} aria-hidden /> Delete</Button>
                </div>
              </div>
            ))
          )}
        </section>
      </div>
    </div>
  );
}
