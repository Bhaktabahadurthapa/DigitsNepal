import { RESPONSE_SCHEMA } from "./prompt";

const API = "https://generativelanguage.googleapis.com/v1beta";

export class UpstreamError extends Error {
  constructor(public kind: "rate_limited" | "auth" | "unavailable" | "bad_output" | "blocked", public retryAfter?: number) {
    super(kind);
  }
}

let cachedModels: { at: number; list: string[] } | null = null;

/** Rank Flash text models newest-first. No model IDs are hard-coded. */
export function rankModels(names: string[]): string[] {
  const bad = /(image|tts|audio|live|embed|vision|thinking|exp|learnlm|robotics|computer|native|aqa|gemma|nano|banana|veo|imagen)/i;
  const version = (n: string) => {
    const m = n.match(/gemini-(\d+(?:\.\d+)?)/);
    return m ? Number.parseFloat(m[1] ?? "0") : 0;
  };
  return names
    .map((n) => n.replace(/^models\//, ""))
    .filter((n) => /^gemini-/.test(n) && /flash/.test(n) && !bad.test(n))
    .sort((a, b) => {
      const dv = version(b) - version(a);
      if (dv) return dv;
      const pa = /preview/.test(a) ? 1 : 0, pb = /preview/.test(b) ? 1 : 0; // stable before preview
      if (pa !== pb) return pa - pb;
      const la = /lite/.test(a) ? 1 : 0, lb = /lite/.test(b) ? 1 : 0; // full Flash before Lite
      if (la !== lb) return la - lb;
      return /latest/.test(a) ? -1 : /latest/.test(b) ? 1 : a.length - b.length;
    });
}

export async function candidateModels(key: string, configured: string | undefined, now = Date.now()): Promise<string[]> {
  if (configured && configured.trim()) return [configured.trim()];
  if (cachedModels && now - cachedModels.at < 6 * 3600_000) return cachedModels.list;
  const r = await fetch(`${API}/models?pageSize=200`, { headers: { "x-goog-api-key": key }, signal: AbortSignal.timeout(10_000) });
  if (r.status === 400 || r.status === 401 || r.status === 403) throw new UpstreamError("auth");
  if (!r.ok) throw new UpstreamError("unavailable");
  const data = (await r.json()) as { models?: { name: string; supportedGenerationMethods?: string[] }[] };
  const names = (data.models ?? []).filter((m) => m.supportedGenerationMethods?.includes("generateContent")).map((m) => m.name);
  const list = rankModels(names).slice(0, 4);
  if (!list.length) throw new UpstreamError("unavailable");
  cachedModels = { at: now, list };
  return list;
}

export function resetModelCache() { cachedModels = null; }

async function callOnce(key: string, model: string, system: string, user: string, schema: boolean): Promise<Response> {
  const generationConfig: Record<string, unknown> = { responseMimeType: "application/json", temperature: 0.2, maxOutputTokens: 4096 };
  if (schema) generationConfig.responseSchema = RESPONSE_SCHEMA;
  return fetch(`${API}/models/${encodeURIComponent(model)}:generateContent`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "x-goog-api-key": key },
    body: JSON.stringify({ systemInstruction: { parts: [{ text: system }] }, contents: [{ role: "user", parts: [{ text: user }] }], generationConfig }),
    signal: AbortSignal.timeout(30_000),
  });
}

/** Generate JSON text, trying the next model when one is rate-limited or missing. */
export async function generate(key: string, models: string[], system: string, user: string): Promise<{ text: string; model: string }> {
  let last: UpstreamError = new UpstreamError("unavailable");
  for (const model of models) {
    let r: Response;
    try {
      r = await callOnce(key, model, system, user, true);
      if (r.status === 400) r = await callOnce(key, model, system, user, false); // model may not accept responseSchema
    } catch {
      last = new UpstreamError("unavailable");
      continue;
    }
    if (r.status === 401 || r.status === 403) throw new UpstreamError("auth");
    if (r.status === 429) {
      const ra = Number.parseInt(r.headers.get("Retry-After") ?? "", 10);
      last = new UpstreamError("rate_limited", Number.isFinite(ra) ? ra : undefined);
      continue;
    }
    if (r.status === 404) { last = new UpstreamError("unavailable"); continue; }
    if (!r.ok) { last = new UpstreamError("unavailable"); continue; }
    const data = (await r.json()) as {
      candidates?: { content?: { parts?: { text?: string; thought?: boolean }[] }; finishReason?: string }[];
      promptFeedback?: { blockReason?: string };
    };
    if (data.promptFeedback?.blockReason) throw new UpstreamError("blocked");
    const c = data.candidates?.[0];
    if (c?.finishReason === "SAFETY") throw new UpstreamError("blocked");
    const text = (c?.content?.parts ?? []).filter((p) => !p.thought).map((p) => p.text ?? "").join("");
    if (!text.trim()) { last = new UpstreamError("bad_output"); continue; }
    return { text, model };
  }
  throw last;
}
