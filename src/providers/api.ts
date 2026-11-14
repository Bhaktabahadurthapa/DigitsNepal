// Typed wrappers around Tauri commands. All network, keychain, clipboard and
// accessibility work happens in Rust; the webview never sees API keys.
import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  Detection,
  HistoryEntry,
  ModelInfo,
  PendingCapture,
  ProviderId,
  ProviderInfo,
  Settings,
  TranslateResponse,
  TranslationMode,
  TranslationResult,
  WordMeaning,
} from "../types/translation";

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e && "code" in e) return e as AppError;
  if (typeof e === "string") return { code: "error", message: e, retryable: false };
  return { code: "error", message: e instanceof Error ? e.message : "Something went wrong.", retryable: false };
}

const demoMode = () => !isTauri() && import.meta.env.DEV;

async function demo(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  const d = await import("./demo");
  switch (cmd) {
    case "get_settings":
      return d.demoSettings;
    case "update_settings":
      return args?.next;
    case "translate":
      await new Promise((r) => setTimeout(r, 400));
      return d.demoTranslate((args?.args as { text: string }).text);
    case "detect_language":
      return d.demoDetection;
    case "dictionary_gloss":
      return d.demoGloss(args?.text as string);
    case "history_search":
      return [];
    case "provider_status":
      return { accessibility_trusted: false };
    case "list_providers":
      return d.demoProviders;
    case "list_models":
      return { models: [{ id: "demo", display_name: "demo", description: "fixture" }], recommended: "demo" };
    case "get_pending_capture":
      return { status: "ok", text: d.DEMO_SOURCE, message: null, can_replace: true, clipboard_fallback: false, seq: 1 };
    default:
      return undefined;
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (demoMode()) return (await demo(cmd, args)) as T;
  if (!isTauri()) {
    throw { code: "not_desktop", message: "Run DigitsNepal as the desktop app (npm run app:dev) to use this feature.", retryable: false } satisfies AppError;
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}

export const api = {
  translate: (text: string, mode: TranslationMode, includeWordMeanings: boolean, isPrivate = false) =>
    call<TranslateResponse>("translate", {
      args: { text, mode, include_word_meanings: includeWordMeanings, private: isPrivate },
    }),
  detectLanguage: (text: string) => call<Detection>("detect_language", { text }),
  dictionaryGloss: (text: string) => call<WordMeaning[]>("dictionary_gloss", { text }),
  listProviders: () => call<ProviderInfo[]>("list_providers"),
  listModels: (provider: ProviderId) => call<{ models: ModelInfo[]; recommended: string | null }>("list_models", { provider }),
  providerStatus: () => call<{ accessibility_trusted: boolean }>("provider_status"),
  setApiKey: (provider: ProviderId, key: string) => call<void>("set_api_key", { provider, key }),
  deleteApiKey: (provider: ProviderId) => call<void>("delete_api_key", { provider }),
  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (next: Settings) => call<Settings>("update_settings", { next }),
  historySearch: (query: string, favoritesOnly: boolean) => call<HistoryEntry[]>("history_search", { query, favoritesOnly }),
  historySave: (result: TranslationResult) => call<HistoryEntry>("history_save", { result }),
  historyFavorite: (id: string, favorite: boolean) => call<boolean>("history_favorite", { id, favorite }),
  historyDelete: (id: string) => call<boolean>("history_delete", { id }),
  historyClear: () => call<void>("history_clear"),
  historyExport: () => call<string>("history_export"),
  copyText: (text: string) => call<void>("copy_text", { text }),
  readClipboardText: () => call<string>("read_clipboard_text"),
  getPendingCapture: () => call<PendingCapture>("get_pending_capture"),
  replaceSelection: (text: string) => call<{ can_undo: boolean }>("replace_selection", { text }),
  undoReplacement: () => call<void>("undo_replacement"),
  dismissPopover: (restoreFocus: boolean) => call<void>("dismiss_popover", { restoreFocus }),
  accessibilityStatus: () => call<{ trusted: boolean }>("accessibility_status"),
  requestAccessibility: () => call<{ trusted: boolean }>("request_accessibility"),
  openMain: (tab?: string) => call<void>("open_main", { tab }),
};
