import { useEffect, useState, type ReactNode } from "react";
import { Button, Notice, Toggle } from "../../components/ui";
import { ModeSelect } from "../../components/ModeSelect";
import { api } from "../../providers/api";
import type { AppError, Settings } from "../../types/translation";
import { ProviderSettings } from "./ProviderSettings";

export function SettingsPage({ settings, update, error }: { settings: Settings | null; update: (p: Partial<Settings>) => Promise<Settings | undefined>; error: AppError | null }) {
  const [status, setStatus] = useState<{ accessibility_trusted: boolean } | null>(null);
  const [shortcut, setShortcut] = useState(settings?.shortcut ?? "");

  const refreshStatus = () => api.providerStatus().then(setStatus).catch(() => {});
  useEffect(() => {
    refreshStatus();
  }, []);
  useEffect(() => {
    if (settings) setShortcut(settings.shortcut);
  }, [settings?.shortcut]);

  if (!settings) return <div className="p-6 text-muted">Loading settings…</div>;
  const s = settings;

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mx-auto max-w-2xl space-y-6">
        {error && <div role="alert" className="rounded-md bg-error-bg px-3 py-2 text-error-text">{error.message}</div>}

        <Section title="AI provider">
          <ProviderSettings settings={s} update={update} />
        </Section>

        <Section title="Translation">
          <div className="py-2">
            <div className="mb-1 text-[12px] text-muted">Default mode</div>
            <ModeSelect value={s.default_mode} onChange={(m) => update({ default_mode: m })} />
          </div>
          <Toggle checked={s.word_meanings} onChange={(v) => update({ word_meanings: v })} label="Include word meanings" description="Ask the model to explain each Nepali word or phrase. Slightly slower." />
        </Section>

        <Section title="Keyboard shortcut & selected text">
          <Toggle checked={s.shortcut_enabled} onChange={(v) => update({ shortcut_enabled: v })} label="Enable global shortcut" description="Select text in any app and press the shortcut to translate it." />
          <div className="flex items-center gap-2 py-2">
            <input aria-label="Shortcut" className="h-7 w-56 rounded-md border border-line bg-surface px-2 font-mono text-[12px]" value={shortcut} onChange={(e) => setShortcut(e.target.value)} />
            <Button onClick={() => update({ shortcut }).catch(() => {})}>Apply</Button>
            <span className="text-[12px] text-muted">e.g. Control+Option+T</span>
          </div>
          <div className="flex items-center justify-between py-2">
            <div>
              <div className="font-medium">Accessibility permission</div>
              <div className="text-[12px] text-muted">
                {status?.accessibility_trusted ? "Granted — DigitsNepal can read the selected text when you press the shortcut." : "Not granted. Needed to read and replace selected text in other apps."}
              </div>
            </div>
            {!status?.accessibility_trusted && (
              <Button onClick={async () => { await api.requestAccessibility(); setTimeout(refreshStatus, 1500); }}>Open System Settings</Button>
            )}
          </div>
          <Notice tone="info">
            To revoke, remove DigitsNepal from System Settings › Privacy & Security › Accessibility. To reset it from Terminal: <code>tccutil reset Accessibility org.digitsnepal.app</code>
          </Notice>
          <Toggle
            checked={s.clipboard_fallback}
            onChange={(v) => update({ clipboard_fallback: v })}
            label="Offer clipboard fallback"
            description="If an app does not expose selected text, show a “Use clipboard text” button. The clipboard is only read when you click it."
          />
        </Section>

        <Section title="Privacy">
          <Toggle checked={s.history_enabled} onChange={(v) => update({ history_enabled: v })} label="Save translation history on this Mac" description="Off by default. Stored locally only; manage it in History." />
          <Toggle checked={s.auto_translate} onChange={(v) => update({ auto_translate: v })} label="Auto-translate as I type" description="Translates on the Translate screen when you pause or finish a sentence. With a cloud provider, each update is an API request." />
          <p className="py-2 text-[12px] text-muted">DigitsNepal has no analytics, no telemetry, and no account. Network requests are only made to the provider you choose.</p>
        </Section>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-muted">{title}</h2>
      <div className="divide-y divide-line rounded-lg border border-line bg-surface px-4">{children}</div>
    </section>
  );
}
