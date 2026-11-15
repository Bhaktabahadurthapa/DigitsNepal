import type { Mode, Quota, Translation } from "./types";

export const API_URL = (import.meta.env.VITE_API_URL ?? "").replace(/\/+$/, "");

export class ApiError extends Error {
  constructor(public code: string, message: string, public status = 0) {
    super(message);
  }
}

async function request<T>(path: string, init: RequestInit & { token?: string } = {}): Promise<T> {
  if (!API_URL) throw new ApiError("not_configured", "The translation service is not set up yet.");
  if (typeof navigator !== "undefined" && navigator.onLine === false) {
    throw new ApiError("offline", "You're offline. Translation needs an internet connection.");
  }
  const headers = new Headers(init.headers);
  if (init.body) headers.set("Content-Type", "application/json");
  if (init.token) headers.set("Authorization", `Bearer ${init.token}`);
  let r: Response;
  try {
    r = await fetch(API_URL + path, { ...init, headers, cache: "no-store", credentials: "omit", referrerPolicy: "no-referrer", signal: AbortSignal.timeout(45_000) });
  } catch (e) {
    if (e instanceof DOMException && (e.name === "TimeoutError" || e.name === "AbortError")) {
      throw new ApiError("timeout", "DigitsNepalAI took too long to answer. Please try again.");
    }
    throw new ApiError("offline", "Can't reach DigitsNepalAI. Check your internet connection.");
  }
  let body: unknown = null;
  try { body = await r.json(); } catch { /* non-JSON */ }
  if (!r.ok) {
    const err = (body as { error?: { code?: string; message?: string } } | null)?.error;
    throw new ApiError(err?.code ?? "http_" + r.status, err?.message ?? "Something went wrong. Please try again.", r.status);
  }
  return body as T;
}

export const signIn = (code: string) =>
  request<{ token: string; expiresAt: number }>("/auth/login", { method: "POST", body: JSON.stringify({ code }) });

export const checkSession = (token: string) =>
  request<{ expiresAt: number; quota: Quota }>("/auth/session", { token });

export const translate = (token: string, text: string, mode: Mode, meanings: boolean) =>
  request<Translation>("/translate", { method: "POST", token, body: JSON.stringify({ text, mode, meanings }) });
