import { b64url, hmac } from "./crypto";
import type { Env } from "./env";

export const utcDay = (now = Date.now()) => new Date(now).toISOString().slice(0, 10);
const nextUtcMidnight = (now = Date.now()) => {
  const d = new Date(now);
  return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + 1);
};

async function count(env: Env, key: string): Promise<number> {
  const v = await env.LIMITS.get(key);
  const n = Number.parseInt(v ?? "0", 10);
  return Number.isFinite(n) ? n : 0;
}

/** Daily translation quota shared by all signed-in devices. Counts before the upstream call. */
export async function takeDailyQuota(env: Env, limit: number, now = Date.now()) {
  const key = `q:${utcDay(now)}`;
  const used = await count(env, key);
  const resetAt = nextUtcMidnight(now);
  if (used >= limit) return { ok: false as const, used, limit, resetAt };
  await env.LIMITS.put(key, String(used + 1), { expirationTtl: 172800 });
  return { ok: true as const, used: used + 1, limit, resetAt };
}

export async function peekDailyQuota(env: Env, limit: number, now = Date.now()) {
  return { used: await count(env, `q:${utcDay(now)}`), limit, resetAt: nextUtcMidnight(now) };
}

// Sign-in throttling. IPs are stored only as keyed hashes.
const PER_IP = 5; // failures per 15 minutes per network
const PER_DAY = 40; // failures per day from everywhere → sign-in locked until tomorrow

async function ipKey(env: Env, ip: string): Promise<string> {
  return "lf:" + b64url((await hmac(env.SESSION_SECRET ?? "", "ip:" + ip)).slice(0, 12));
}

export async function loginBlocked(env: Env, ip: string, now = Date.now()): Promise<boolean> {
  const [a, b] = await Promise.all([count(env, await ipKey(env, ip)), count(env, `lfd:${utcDay(now)}`)]);
  return a >= PER_IP || b >= PER_DAY;
}

export async function recordLoginFailure(env: Env, ip: string, now = Date.now()): Promise<void> {
  const k = await ipKey(env, ip);
  const d = `lfd:${utcDay(now)}`;
  const [a, b] = await Promise.all([count(env, k), count(env, d)]);
  await Promise.all([
    env.LIMITS.put(k, String(a + 1), { expirationTtl: 900 }),
    env.LIMITS.put(d, String(b + 1), { expirationTtl: 172800 }),
  ]);
}

export async function burstOk(env: Env, key: string): Promise<boolean> {
  if (!env.BURST) return true;
  try {
    return (await env.BURST.limit({ key })).success;
  } catch {
    return true;
  }
}
