import { describe, expect, it } from "vitest";
import { nextStep, setupSteps } from "./providerReady";
import type { ProviderInfo, Settings } from "../types/translation";

const custom: ProviderInfo = {
  id: "custom", name: "Custom (OpenAI-compatible)", protocol: "open_ai", default_base_url: "", local: false,
  needs_key: false, editable_url: true, key_help: "", key_present: true, base_url: "https://api.example.com/v1", cloud: true,
};
const settings = (consent: boolean, model: string): Settings => ({
  provider: "custom", providers: { custom: { model, base_url: "https://api.example.com/v1", cloud_consent: consent } },
  default_mode: "accurate", shortcut: "Control+Alt+T", shortcut_enabled: true, history_enabled: false,
  clipboard_fallback: false, word_meanings: true, auto_translate: true, saved_keys: [], hide_from_capture: false, window_opacity: 100, paused: false, typing_button: false, instant_replace: false, onboarding_done: false,
});

describe("provider setup steps", () => {
  it("flags missing consent first, then model (the reported bug)", () => {
    expect(nextStep(custom, settings(false, ""))?.id).toBe("consent");
    expect(nextStep(custom, settings(true, ""))?.id).toBe("model");
    expect(nextStep(custom, settings(true, "m"))).toBeUndefined();
  });

  it("local providers never ask for consent or a key", () => {
    const ollama = { ...custom, id: "ollama", local: true, cloud: false, editable_url: true, base_url: "http://127.0.0.1:11434" };
    const s = { ...settings(false, ""), provider: "ollama", providers: {} };
    expect(setupSteps(ollama, s).map((x) => x.id)).toEqual(["address", "model"]);
  });

  it("requires a key where the preset needs one", () => {
    const openai = { ...custom, id: "openai", needs_key: true, key_present: false, editable_url: false };
    const s = { ...settings(true, "m"), provider: "openai", providers: { openai: { model: "m", base_url: "", cloud_consent: true } } };
    expect(nextStep(openai, s)?.id).toBe("key");
  });
});
