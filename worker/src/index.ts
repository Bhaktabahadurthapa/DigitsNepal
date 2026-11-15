import { bearer, codeMatches, issueToken, verifyToken } from "./auth";
import { configProblem, num, type Env } from "./env";
import { candidateModels, generate, UpstreamError } from "./gemini";
import { allowedOrigin, fail, json, preflight, readJson } from "./http";
import { burstOk, loginBlocked, peekDailyQuota, recordLoginFailure, takeDailyQuota } from "./limits";
import { buildPrompt, MODES, type Mode } from "./prompt";
import { extractJson, validate } from "./validate";

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    try {
      return await route(req, env);
    } catch (e) {
      // Log only the error class, never request content.
      console.error("unhandled", e instanceof Error ? e.name : "unknown");
      return fail(500, "internal", "Something went wrong. Please try again.", allowedOrigin(req, env));
    }
  },
} satisfies ExportedHandler<Env>;

async function route(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);
  const origin = allowedOrigin(req, env);

  if (req.method === "OPTIONS") return preflight(origin);
  if (url.pathname === "/health" && req.method === "GET") return json({ ok: true }, 200, origin);

  // Every real endpoint is browser-only from the allowed origins.
  if (!origin) return fail(403, "origin", "Requests are only accepted from digitsnepal.com.", null);

  const problem = configProblem(env);
  if (problem) {
    console.error("config", problem);
    return fail(503, "not_configured", "The translation service is not set up yet.", origin);
  }
  const secret = env.SESSION_SECRET as string;
  const ip = req.headers.get("CF-Connecting-IP") ?? "unknown";

  if (url.pathname === "/auth/login" && req.method === "POST") {
    if (await loginBlocked(env, ip)) return fail(429, "login_locked", "Too many attempts. Try again later.", origin, { "Retry-After": "900" });
    const body = await readJson(req);
    const code = typeof body?.code === "string" ? body.code : "";
    if (!code || code.length > 200 || !(await codeMatches(code, env.ACCESS_CODE as string, secret))) {
      await recordLoginFailure(env, ip);
      return fail(401, "bad_code", "That access code is not correct.", origin);
    }
    const t = await issueToken(secret, num(env.SESSION_DAYS, 180, 1, 365));
    return json(t, 200, origin);
  }

  const session = await verifyToken(bearer(req), secret);

  if (url.pathname === "/auth/session" && req.method === "GET") {
    if (!session) return fail(401, "signed_out", "Please sign in.", origin);
    const quota = await peekDailyQuota(env, num(env.DAILY_LIMIT, 150, 1, 1000));
    // Sliding sign-in: a device that keeps using the app is re-issued a fresh token
    // (at most once a day), so it never has to type the access code again.
    if (Date.now() - session.iat * 1000 > 86400_000) {
      const renewed = await issueToken(secret, num(env.SESSION_DAYS, 180, 1, 365));
      return json({ ...renewed, quota }, 200, origin);
    }
    return json({ expiresAt: session.exp * 1000, quota }, 200, origin);
  }

  if (url.pathname === "/translate" && req.method === "POST") {
    if (!session) return fail(401, "signed_out", "Please sign in.", origin);
    if (!(await burstOk(env, session.sid))) return fail(429, "slow_down", "You're going a little fast. Wait a few seconds.", origin, { "Retry-After": "10" });

    const body = await readJson(req);
    if (!body) return fail(400, "bad_request", "The request could not be read.", origin);
    const text = typeof body.text === "string" ? body.text.trim() : "";
    const maxChars = num(env.MAX_CHARS, 2000, 50, 6000);
    if (!text) return fail(400, "empty", "Type something to translate.", origin);
    if ([...text].length > maxChars) return fail(413, "too_long", `Please keep it under ${maxChars} characters.`, origin);
    const mode: Mode = MODES.includes(body.mode as Mode) ? (body.mode as Mode) : "accurate";
    const meanings = body.meanings !== false;

    const quota = await takeDailyQuota(env, num(env.DAILY_LIMIT, 150, 1, 1000));
    if (!quota.ok) {
      return fail(429, "daily_limit", "Today's translation limit is used up. It resets at midnight UTC.", origin, {
        "Retry-After": String(Math.ceil((quota.resetAt - Date.now()) / 1000)),
      });
    }

    const key = env.GEMINI_API_KEY as string;
    const { system, user } = buildPrompt(text, mode, meanings);
    try {
      const models = await candidateModels(key, env.GEMINI_MODEL);
      let out = await generate(key, models, system, user);
      let result = validate(extractJson(out.text), text, mode);
      if (!result) {
        out = await generate(key, [out.model], system, user + "\n\nReply again with ONLY the JSON object.");
        result = validate(extractJson(out.text), text, mode);
      }
      if (!result) return fail(502, "bad_output", "DigitsNepalAI returned an unusable answer. Please try again.", origin);
      return json({ ...result, mode, model: out.model, quota: { used: quota.used, limit: quota.limit, resetAt: quota.resetAt } }, 200, origin);
    } catch (e) {
      if (e instanceof UpstreamError) {
        console.error("upstream", e.kind);
        switch (e.kind) {
          case "rate_limited":
            return fail(429, "upstream_busy", "Gemini's free tier is busy or used up for now. Try again in a minute.", origin, { "Retry-After": String(e.retryAfter ?? 60) });
          case "auth":
            return fail(503, "upstream_auth", "The Gemini API key was rejected. The owner needs to update it.", origin);
          case "blocked":
            return fail(422, "blocked", "This text couldn't be translated by the model.", origin);
          default:
            return fail(502, "upstream_unavailable", "DigitsNepalAI is unavailable right now. Please try again.", origin);
        }
      }
      throw e;
    }
  }

  return fail(404, "not_found", "Not found.", origin);
}
