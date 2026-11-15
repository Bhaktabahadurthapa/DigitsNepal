export interface Env {
  // Secrets (wrangler secret put …)
  GEMINI_API_KEY?: string;
  ACCESS_CODE?: string;
  SESSION_SECRET?: string;
  // Vars (wrangler.toml)
  ALLOWED_ORIGINS?: string;
  DAILY_LIMIT?: string;
  MAX_CHARS?: string;
  SESSION_DAYS?: string;
  GEMINI_MODEL?: string;
  // Bindings
  LIMITS: KVNamespace;
  BURST?: { limit(opts: { key: string }): Promise<{ success: boolean }> };
}

export const num = (v: string | undefined, fallback: number, min: number, max: number): number => {
  const n = Number.parseInt(v ?? "", 10);
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
};

/** Secrets must exist and be long enough to resist guessing. */
export function configProblem(env: Env): string | null {
  if (!env.GEMINI_API_KEY) return "GEMINI_API_KEY is not set";
  if (!env.ACCESS_CODE || env.ACCESS_CODE.length < 12) return "ACCESS_CODE must be at least 12 characters";
  if (!env.SESSION_SECRET || env.SESSION_SECRET.length < 32) return "SESSION_SECRET must be at least 32 characters";
  return null;
}
