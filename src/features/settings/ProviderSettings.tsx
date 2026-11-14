import { useCallback, useEffect, useState } from "react";
import { Button, Toggle } from "../../components/ui";
import { api } from "../../providers/api";
import type { AppError, ModelInfo, ProviderConfig, ProviderInfo, Settings } from "../../types/translation";

const EMPTY: ProviderConfig = { model: "", base_url: "", cloud_consent: false };

/** Choose any provider: local (Ollama, LM Studio), cloud presets, or a custom OpenAI-compatible server. */
export function ProviderSettings({ settings, update }: { settings: Settings; update: (p: Partial<Settings>) => Promise<Settings | undefined> }) {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [recommended, setRecommended] = useState<string | null>(null);
  const [modelError, setModelError] = useState<string | null>(null);
  const [key, setKey] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [url, setUrl] = useState("");
  const [manualModel, setManualModel] = useState("");

  const id = settings.provider;
  const cfg = settings.providers[id] ?? EMPTY;
  const info = providers.find((p) => p.id === id);

  const refresh = useCallback(() => api.listProviders().then(setProviders).catch(() => {}), []);
  useEffect(() => {
    refresh();
  }, [refresh, settings]);

  useEffect(() => {
    setUrl(cfg.base_url || info?.default_base_url || "");
    setManualModel(cfg.model);
    setKey("");
    setMsg(null);
  }, [id, info?.default_base_url, cfg.base_url, cfg.model]);

  const setCfg = (patch: Partial<ProviderConfig>) => update({ providers: { ...settings.providers, [id]: { ...cfg, ...patch } } });

  const discover = async () => {
    setModelError(null);
    setModels([]);
    try {
      const r = await api.listModels(id);
      setModels(r.models);
      setRecommended(r.recommended);
      if (!cfg.model && r.recommended) await setCfg({ model: r.recommended });
    } catch (e) {
      setModelError((e as AppError).message);
    }
  };

  const ready = info && (!info.cloud || cfg.cloud_consent) && (!info.needs_key || info.key_present);
  useEffect(() => {
    if (ready) discover();
    else setModels([]);
  }, [id, ready, info?.base_url]);

  const local = providers.filter((p) => p.local);
  const cloud = providers.filter((p) => !p.local);

  return (
    <div className="space-y-3 py-3">
      <div>
        <label className="mb-1 block text-[12px] text-muted" htmlFor="provider">
          Provider
        </label>
        <select id="provider" className="h-8 w-full rounded-md border border-line bg-surface px-2" value={id} onChange={(e) => update({ provider: e.target.value })}>
          <optgroup label="On this Mac — text never leaves your computer">
            {local.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </optgroup>
          <optgroup label="Cloud — bring your own API key">
            {cloud.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
                {p.key_present ? "  ✓ key" : ""}
              </option>
            ))}
          </optgroup>
        </select>
        {info && <p className="mt-1 text-[12px] text-muted">{info.key_help}</p>}
      </div>

      {info?.editable_url && (
        <div>
          <label className="mb-1 block text-[12px] text-muted" htmlFor="base-url">
            Server address {info.local ? "(this Mac only)" : "(OpenAI-compatible, ending in /v1)"}
          </label>
          <div className="flex gap-2">
            <input id="base-url" className="h-7 flex-1 rounded-md border border-line bg-surface px-2 font-mono text-[12px]" placeholder={info.default_base_url || "https://your-server.example/v1"} value={url} onChange={(e) => setUrl(e.target.value)} />
            <Button onClick={() => setCfg({ base_url: url }).catch(() => {})}>Save</Button>
          </div>
        </div>
      )}

      {info?.cloud && (
        <Toggle
          checked={cfg.cloud_consent}
          onChange={(v) => setCfg({ cloud_consent: v })}
          label={`I understand my text is sent to ${info.name}`}
          description="Required before DigitsNepal makes any request to this service. Applies to this provider only."
        />
      )}

      {info && (info.needs_key || info.id === "custom" || info.protocol === "open_ai") && !info.local && (
        <div>
          <label className="mb-1 block text-[12px] text-muted" htmlFor="api-key">
            API key {info.needs_key ? "" : "(optional)"} {info.key_present ? <span className="text-ok">· stored in Keychain</span> : <span>· not set</span>}
          </label>
          <div className="flex gap-2">
            <input id="api-key" type="password" autoComplete="off" className="h-7 flex-1 rounded-md border border-line bg-surface px-2" placeholder={info.key_present ? "••••••••  (replace key)" : "Paste API key"} value={key} onChange={(e) => setKey(e.target.value)} />
            <Button
              disabled={!key}
              onClick={async () => {
                try {
                  await api.setApiKey(id, key);
                  setKey("");
                  setMsg("Saved to macOS Keychain.");
                  refresh();
                } catch (e) {
                  setMsg((e as AppError).message);
                }
              }}
            >
              Save key
            </Button>
            {info.key_present && (
              <Button variant="danger" onClick={async () => { await api.deleteApiKey(id); setMsg("Key removed."); refresh(); }}>
                Remove
              </Button>
            )}
          </div>
          {msg && <p className="mt-1 text-[12px] text-muted">{msg}</p>}
        </div>
      )}

      <div>
        <div className="mb-1 flex items-center justify-between">
          <label className="text-[12px] text-muted" htmlFor="model">
            Model
          </label>
          <Button variant="ghost" onClick={discover} disabled={!ready}>
            Refresh models
          </Button>
        </div>
        {models.length > 0 && (
          <select id="model" className="mb-2 h-7 w-full rounded-md border border-line bg-surface px-2" value={models.some((m) => m.id === cfg.model) ? cfg.model : ""} onChange={(e) => setCfg({ model: e.target.value })}>
            <option value="">Choose from {models.length} models…</option>
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.display_name !== m.id ? `${m.display_name} — ${m.id}` : m.id}
                {m.id === recommended ? "  (suggested)" : ""}
                {m.description ? `  · ${m.description}` : ""}
              </option>
            ))}
          </select>
        )}
        <div className="flex gap-2">
          <input aria-label="Model id" className="h-7 flex-1 rounded-md border border-line bg-surface px-2 font-mono text-[12px]" placeholder="or type a model id" value={manualModel} onChange={(e) => setManualModel(e.target.value)} />
          <Button disabled={manualModel.trim() === cfg.model} onClick={() => setCfg({ model: manualModel })}>
            Use
          </Button>
        </div>
        {!ready && info && <p className="mt-1 text-[12px] text-muted">{info.cloud && !cfg.cloud_consent ? "Confirm cloud use above to load models." : info.needs_key && !info.key_present ? "Add an API key to load models." : ""}</p>}
        {modelError && <p className="mt-1 text-[12px] text-error-text">{modelError}</p>}
        {id === "ollama" && !modelError && ready && models.length === 0 && (
          <p className="mt-1 text-[12px] text-muted">
            No local models yet. In Terminal: <code>ollama pull gemma3</code> (or another multilingual model).
          </p>
        )}
      </div>
    </div>
  );
}
