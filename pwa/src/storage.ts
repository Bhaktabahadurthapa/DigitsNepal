import type { Appearance, HistoryItem } from "./types";

// localStorage can throw (private mode, storage disabled). Never let that break the app.
export function load<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}
export function save(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch { /* ignore */ }
}
export function remove(key: string): void {
  try { localStorage.removeItem(key); } catch { /* ignore */ }
}

export interface Prefs { appearance: Appearance; meanings: boolean; history: boolean; live: boolean }
export const DEFAULT_PREFS: Prefs = { appearance: "system", meanings: true, history: false, live: true };
export const loadPrefs = (): Prefs => ({ ...DEFAULT_PREFS, ...load<Partial<Prefs>>("dn.prefs", {}) });
export const savePrefs = (p: Prefs) => save("dn.prefs", p);

export interface Session { token: string; expiresAt: number }
export function loadSession(): Session | null {
  const s = load<Session | null>("dn.session", null);
  return s && typeof s.token === "string" && s.expiresAt > Date.now() ? s : null;
}
export const saveSession = (s: Session) => save("dn.session", s);
export const clearSession = () => remove("dn.session");

const HISTORY_MAX = 50;
export const loadHistory = (): HistoryItem[] => load<HistoryItem[]>("dn.history", []);
export function addHistory(item: HistoryItem): HistoryItem[] {
  let prev = loadHistory();
  // Live typing produces many partial versions of one sentence: keep only the latest.
  const top = prev[0];
  if (top && top.mode === item.mode && item.at - top.at < 120_000 && (item.text.startsWith(top.text) || top.text.startsWith(item.text))) {
    prev = prev.slice(1);
  }
  const list = [item, ...prev.filter((h) => !(h.text === item.text && h.mode === item.mode))].slice(0, HISTORY_MAX);
  save("dn.history", list);
  return list;
}
export const clearHistory = () => remove("dn.history");

/** Ask the browser not to evict our storage (keeps the sign-in). Best effort. */
export function requestPersistence(): void {
  try {
    void navigator.storage?.persist?.();
  } catch { /* ignore */ }
}
