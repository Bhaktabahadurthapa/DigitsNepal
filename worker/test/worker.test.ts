import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import worker from "../src/index";
import { issueToken, verifyToken } from "../src/auth";
import type { Env } from "../src/env";
import { rankModels, resetModelCache } from "../src/gemini";
import { validate } from "../src/validate";

const ORIGIN = "https://digitsnepal.com";
const CODE = "correct-horse-battery-staple";
const SECRET = "s".repeat(48);

class FakeKV {
  store = new Map<string, string>();
  async get(k: string) { return this.store.get(k) ?? null; }
  async put(k: string, v: string) { this.store.set(k, v); }
}

function makeEnv(over: Partial<Env> = {}): Env {
  return {
    GEMINI_API_KEY: "test-gemini-key",
    ACCESS_CODE: CODE,
    SESSION_SECRET: SECRET,
    ALLOWED_ORIGINS: "https://digitsnepal.com",
    DAILY_LIMIT: "3",
    MAX_CHARS: "200",
    GEMINI_MODEL: "",
    LIMITS: new FakeKV() as unknown as KVNamespace,
    ...over,
  };
}

const req = (path: string, init: RequestInit & { origin?: string | null; token?: string } = {}) => {
  const h = new Headers(init.headers);
  if (init.origin !== null) h.set("Origin", init.origin ?? ORIGIN);
  if (init.token) h.set("Authorization", `Bearer ${init.token}`);
  if (init.body) h.set("Content-Type", "application/json");
  h.set("CF-Connecting-IP", "203.0.113.7");
  return new Request("https://api.example.workers.dev" + path, { ...init, headers: h });
};
const call = (r: Request, env: Env) => worker.fetch(r, env, {} as ExecutionContext);

const GOOD = {
  detected_language: "romanized_nepali",
  nepali_script: "मलाई थाहा छैन।",
  accurate_english: "I don't know.",
  styled_english: "I am not sure.",
  grammar_notes: [],
  word_meanings: [
    { source: "Malai", devanagari: "मलाई", meaning: "to me", literal: null, note: "dative", uncertain: false },
    { source: "invented", devanagari: null, meaning: "x", literal: null, note: null, uncertain: false },
  ],
  ambiguities: [],
};

function mockGemini(handler: (url: string, init?: RequestInit) => Response) {
  const f = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => handler(String(input), init));
  vi.stubGlobal("fetch", f);
  return f;
}
const modelsList = () =>
  new Response(JSON.stringify({ models: [
    { name: "models/gemini-9.0-flash", supportedGenerationMethods: ["generateContent"] },
    { name: "models/gemini-9.0-flash-lite", supportedGenerationMethods: ["generateContent"] },
    { name: "models/gemini-9.0-flash-image", supportedGenerationMethods: ["generateContent"] },
    { name: "models/text-embedding-9", supportedGenerationMethods: ["embedContent"] },
  ] }));
const gen = (obj: unknown) => new Response(JSON.stringify({ candidates: [{ content: { parts: [{ text: JSON.stringify(obj) }] }, finishReason: "STOP" }] }));

async function login(env: Env) {
  const r = await call(req("/auth/login", { method: "POST", body: JSON.stringify({ code: CODE }) }), env);
  return ((await r.json()) as { token: string }).token;
}

beforeEach(() => resetModelCache());
afterEach(() => vi.unstubAllGlobals());

describe("access control", () => {
  it("rejects requests from other origins and without an origin", async () => {
    const env = makeEnv();
    expect((await call(req("/translate", { method: "POST", origin: "https://evil.example", body: "{}" }), env)).status).toBe(403);
    expect((await call(req("/translate", { method: "POST", origin: null, body: "{}" }), env)).status).toBe(403);
    expect((await call(req("/translate", { method: "OPTIONS", origin: "https://evil.example" }), env)).status).toBe(403);
  });

  it("preflight allows only the configured origin", async () => {
    const r = await call(req("/translate", { method: "OPTIONS" }), makeEnv());
    expect(r.status).toBe(204);
    expect(r.headers.get("Access-Control-Allow-Origin")).toBe(ORIGIN);
  });

  it("refuses to run with weak or missing secrets", async () => {
    const r = await call(req("/auth/login", { method: "POST", body: JSON.stringify({ code: "short" }) }), makeEnv({ ACCESS_CODE: "short" }));
    expect(r.status).toBe(503);
  });

  it("rejects a wrong code and locks after repeated failures", async () => {
    const env = makeEnv();
    for (let i = 0; i < 5; i++) {
      const r = await call(req("/auth/login", { method: "POST", body: JSON.stringify({ code: "wrong-" + i }) }), env);
      expect(r.status).toBe(401);
    }
    const locked = await call(req("/auth/login", { method: "POST", body: JSON.stringify({ code: CODE }) }), env);
    expect(locked.status).toBe(429);
    // KV never stores the raw IP address.
    const keys = [...(env.LIMITS as unknown as FakeKV).store.keys()].join(" ");
    expect(keys).not.toContain("203.0.113.7");
  });

  it("issues a token for the right code and accepts it", async () => {
    const env = makeEnv();
    const token = await login(env);
    expect(token).toMatch(/^v1\./);
    const s = await call(req("/auth/session", { token }), env);
    expect(s.status).toBe(200);
    expect(JSON.stringify(await s.json())).not.toContain(CODE);
  });

  it("rejects missing, tampered, foreign and expired tokens", async () => {
    const env = makeEnv();
    const token = await login(env);
    const body = JSON.stringify({ text: "Malai thaha chaina" });
    expect((await call(req("/translate", { method: "POST", body }), env)).status).toBe(401);
    const tampered = token.slice(0, -2) + (token.endsWith("AA") ? "BB" : "AA");
    expect((await call(req("/translate", { method: "POST", body, token: tampered }), env)).status).toBe(401);
    const foreign = (await issueToken("x".repeat(48), 30)).token;
    expect((await call(req("/translate", { method: "POST", body, token: foreign }), env)).status).toBe(401);
    const old = (await issueToken(SECRET, 1, Date.now() - 3 * 86400_000)).token;
    expect(await verifyToken(old, SECRET)).toBeNull();
    expect((await call(req("/translate", { method: "POST", body, token: old }), env)).status).toBe(401);
  });
});

describe("translate", () => {
  it("translates, validates and never caches", async () => {
    const env = makeEnv();
    const token = await login(env);
    const f = mockGemini((url) => (url.includes("/models?") ? modelsList() : gen(GOOD)));
    const r = await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "Malai thaha chaina", mode: "professional" }) }), env);
    expect(r.status).toBe(200);
    expect(r.headers.get("Cache-Control")).toBe("no-store");
    const j = (await r.json()) as Record<string, unknown>;
    expect(j.accurate_english).toBe("I don't know.");
    expect(j.model).toBe("gemini-9.0-flash");
    expect((j.word_meanings as unknown[]).length).toBe(1); // invented entry dropped
    // The Gemini key goes in a header, never in the URL.
    for (const [u, init] of f.mock.calls) {
      expect(String(u)).not.toContain("test-gemini-key");
      expect(new Headers(init?.headers).get("x-goog-api-key")).toBe("test-gemini-key");
    }
  });

  it("falls back to the next model when one is rate-limited", async () => {
    const env = makeEnv();
    const token = await login(env);
    mockGemini((url) => (url.includes("/models?") ? modelsList() : url.includes("flash-lite") ? gen(GOOD) : new Response("{}", { status: 429 })));
    const r = await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "Malai thaha chaina" }) }), env);
    expect(r.status).toBe(200);
    expect(((await r.json()) as { model: string }).model).toBe("gemini-9.0-flash-lite");
  });

  it("reports upstream rate limits and bad keys clearly", async () => {
    const env = makeEnv({ DAILY_LIMIT: "50" });
    const token = await login(env);
    mockGemini((url) => (url.includes("/models?") ? modelsList() : new Response("{}", { status: 429 })));
    const r = await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "namaste" }) }), env);
    expect(r.status).toBe(429);
    expect(((await r.json()) as { error: { code: string } }).error.code).toBe("upstream_busy");
    resetModelCache();
    mockGemini(() => new Response("{}", { status: 403 }));
    const r2 = await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "namaste" }) }), env);
    expect(r2.status).toBe(503);
  });

  it("enforces the daily quota", async () => {
    const env = makeEnv({ DAILY_LIMIT: "2" });
    const token = await login(env);
    mockGemini((url) => (url.includes("/models?") ? modelsList() : gen(GOOD)));
    const body = JSON.stringify({ text: "Malai thaha chaina" });
    expect((await call(req("/translate", { method: "POST", token, body }), env)).status).toBe(200);
    expect((await call(req("/translate", { method: "POST", token, body }), env)).status).toBe(200);
    const r = await call(req("/translate", { method: "POST", token, body }), env);
    expect(r.status).toBe(429);
    expect(((await r.json()) as { error: { code: string } }).error.code).toBe("daily_limit");
  });

  it("enforces text and body size limits", async () => {
    const env = makeEnv();
    const token = await login(env);
    expect((await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "a".repeat(201) }) }), env)).status).toBe(413);
    expect((await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "a".repeat(20000) }) }), env)).status).toBe(400);
    expect((await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "   " }) }), env)).status).toBe(400);
  });

  it("does not log user text on failures", async () => {
    const env = makeEnv();
    const token = await login(env);
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockGemini(() => { throw new Error("network down"); });
    await call(req("/translate", { method: "POST", token, body: JSON.stringify({ text: "gopya kura secret-phrase" }) }), env);
    expect(JSON.stringify(spy.mock.calls)).not.toContain("secret-phrase");
    spy.mockRestore();
  });
});

describe("helpers", () => {
  it("ranks Flash text models newest first, stable before preview", () => {
    expect(rankModels(["models/gemini-2.5-flash", "models/gemini-3.0-flash-preview", "models/gemini-3.0-flash", "models/gemini-3.0-flash-lite", "models/gemini-3.0-pro", "models/gemini-3.0-flash-image"]))
      .toEqual(["gemini-3.0-flash", "gemini-3.0-flash-lite", "gemini-3.0-flash-preview", "gemini-2.5-flash"]);
  });

  it("flags a dropped negation and a missing number", () => {
    const v = validate({ ...GOOD, accurate_english: "I know 4 things.", styled_english: null }, "Malai 5 kura thaha chaina", "accurate");
    expect(v?.warnings.join(" ")).toMatch(/negative/);
    expect(v?.warnings.join(" ")).toMatch(/number 5/);
  });

  it("grammar mode keeps notes and drops styled text", () => {
    const v = validate({ ...GOOD, grammar_notes: [{ original: "He go", corrected: "He goes", explanation: "Third person singular takes -s." }] }, "He go", "grammar");
    expect(v?.grammar_notes).toHaveLength(1);
    expect(v?.styled_english).toBeNull();
  });

  it("rejects output without a translation", () => {
    expect(validate({ detected_language: "english" }, "x", "accurate")).toBeNull();
  });
});
