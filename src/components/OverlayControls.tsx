import { api } from "../providers/api";
import type { Settings } from "../types/translation";

/** Opacity slider + "hide from screen sharing" switch, used in the overlay and Settings. */
export function OverlayControls({ settings, compact }: { settings: Settings; compact?: boolean }) {
  const save = (patch: Partial<Settings>) => api.updateSettings({ ...settings, ...patch }).catch(() => {});
  return (
    <div className={`flex items-center ${compact ? "gap-3" : "flex-wrap gap-6 py-2"}`}>
      <label className="flex items-center gap-2 text-[12px] text-muted" title="Background transparency — text stays readable">
        {compact ? "◐" : "Window opacity"}
        <input
          type="range"
          min={30}
          max={100}
          step={5}
          value={settings.window_opacity}
          onChange={(e) => save({ window_opacity: Number(e.target.value) })}
          aria-label="Window opacity"
          className={`${compact ? "w-20" : "w-40"} accent-[var(--accent)]`}
        />
        {!compact && <span className="w-9 tabular-nums">{settings.window_opacity}%</span>}
      </label>
      <label
        className={`flex items-center gap-1.5 whitespace-nowrap text-[12px] ${settings.hide_from_capture ? "text-ok" : "text-muted"}`}
        title="Exclude DigitsNepal from screenshots, recordings and screen sharing"
      >
        <input
          type="checkbox"
          checked={settings.hide_from_capture}
          onChange={(e) => save({ hide_from_capture: e.target.checked })}
          className="accent-[var(--accent)]"
        />
        {compact ? "Hide in share" : "Hide from screen sharing & recordings"}
      </label>
    </div>
  );
}
