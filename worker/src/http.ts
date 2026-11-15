import type { Env } from "./env";

export function allowedOrigin(req: Request, env: Env): string | null {
  const origin = req.headers.get("Origin");
  if (!origin) return null;
  const list = (env.ALLOWED_ORIGINS ?? "").split(",").map((s) => s.trim()).filter(Boolean);
  return list.includes(origin) ? origin : null;
}

export function json(body: unknown, status: number, origin: string | null, extra: Record<string, string> = {}): Response {
  const headers: Record<string, string> = {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
    "Referrer-Policy": "no-referrer",
    Vary: "Origin",
    ...extra,
  };
  if (origin) headers["Access-Control-Allow-Origin"] = origin;
  return new Response(JSON.stringify(body), { status, headers });
}

export const fail = (status: number, code: string, message: string, origin: string | null, extra: Record<string, string> = {}) =>
  json({ error: { code, message } }, status, origin, extra);

export function preflight(origin: string | null): Response {
  if (!origin) return new Response(null, { status: 403 });
  return new Response(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": origin,
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Authorization, Content-Type",
      "Access-Control-Max-Age": "86400",
      Vary: "Origin",
    },
  });
}

export const MAX_BODY_BYTES = 16 * 1024;

/** Read a JSON body with a hard size cap. Returns null on any problem. */
export async function readJson(req: Request): Promise<Record<string, unknown> | null> {
  if (!(req.headers.get("Content-Type") ?? "").toLowerCase().startsWith("application/json")) return null;
  const declared = Number(req.headers.get("Content-Length") ?? "0");
  if (declared > MAX_BODY_BYTES) return null;
  const buf = await req.arrayBuffer();
  if (buf.byteLength > MAX_BODY_BYTES) return null;
  try {
    const v = JSON.parse(new TextDecoder().decode(buf));
    return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}
