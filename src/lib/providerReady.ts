import type { ProviderInfo, Settings } from "../types/translation";

export type SetupStep = { id: "address" | "consent" | "key" | "model"; label: string; done: boolean };

/** What still has to be done before the active provider can translate. */
export function setupSteps(info: ProviderInfo | undefined, settings: Settings): SetupStep[] {
  if (!info) return [];
  const cfg = settings.providers[info.id] ?? { model: "", base_url: "", cloud_consent: false };
  const steps: SetupStep[] = [];
  if (info.editable_url) steps.push({ id: "address", label: "Server address saved", done: !!info.base_url });
  if (info.cloud) steps.push({ id: "consent", label: `Allow sending text to ${info.name}`, done: cfg.cloud_consent });
  if (info.needs_key) steps.push({ id: "key", label: "API key saved", done: info.key_present });
  steps.push({ id: "model", label: "Model chosen", done: !!cfg.model.trim() });
  return steps;
}

export function nextStep(info: ProviderInfo | undefined, settings: Settings): SetupStep | undefined {
  return setupSteps(info, settings).find((s) => !s.done);
}
