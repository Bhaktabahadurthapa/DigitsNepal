import { b64url, fromB64url, hmac, hmacVerify, sameBytes } from "./crypto";

/**
 * Personal access: one long access code (a Worker secret) is exchanged for a
 * signed, expiring device token. Nothing secret ships in the web app, tokens
 * are stateless, and rotating SESSION_SECRET signs every device out.
 */
export interface Session { iat: number; exp: number; sid: string }

export async function codeMatches(attempt: string, accessCode: string, secret: string): Promise<boolean> {
  // Compare HMAC digests so timing never depends on the code's contents or length.
  const [a, b] = await Promise.all([hmac(secret, "code:" + attempt), hmac(secret, "code:" + accessCode)]);
  return sameBytes(a, b);
}

export async function issueToken(secret: string, days: number, now = Date.now()): Promise<{ token: string; expiresAt: number }> {
  const sid = b64url(crypto.getRandomValues(new Uint8Array(9)));
  const s: Session = { iat: Math.floor(now / 1000), exp: Math.floor(now / 1000) + days * 86400, sid };
  const payload = b64url(new TextEncoder().encode(JSON.stringify(s)));
  const sig = b64url(await hmac(secret, "v1." + payload));
  return { token: `v1.${payload}.${sig}`, expiresAt: s.exp * 1000 };
}

export async function verifyToken(token: string | null, secret: string, now = Date.now()): Promise<Session | null> {
  if (!token || token.length > 512) return null;
  const parts = token.split(".");
  if (parts.length !== 3 || parts[0] !== "v1") return null;
  const sig = fromB64url(parts[2] ?? "");
  const raw = fromB64url(parts[1] ?? "");
  if (!sig || !raw) return null;
  if (!(await hmacVerify(secret, "v1." + parts[1], sig))) return null;
  try {
    const s = JSON.parse(new TextDecoder().decode(raw)) as Session;
    if (typeof s.exp !== "number" || s.exp * 1000 <= now) return null;
    return s;
  } catch {
    return null;
  }
}

export function bearer(req: Request): string | null {
  const h = req.headers.get("Authorization") ?? "";
  return h.startsWith("Bearer ") ? h.slice(7).trim() : null;
}
