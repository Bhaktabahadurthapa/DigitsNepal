import { MODES, type TranslationMode } from "../types/translation";

export function ModeSelect({ value, onChange, compact }: { value: TranslationMode; onChange: (m: TranslationMode) => void; compact?: boolean }) {
  if (compact) {
    return (
      <select
        aria-label="Translation mode"
        className="h-7 rounded-md border border-line bg-surface px-2 text-[12.5px]"
        value={value}
        onChange={(e) => onChange(e.target.value as TranslationMode)}
      >
        {MODES.map((m) => (
          <option key={m.id} value={m.id}>
            {m.label}
          </option>
        ))}
      </select>
    );
  }
  return (
    <div role="radiogroup" aria-label="Translation mode" className="inline-flex rounded-md border border-line bg-surface p-0.5">
      {MODES.map((m) => (
        <button
          key={m.id}
          type="button"
          role="radio"
          aria-checked={value === m.id}
          title={m.hint}
          onClick={() => onChange(m.id)}
          className={`h-6 rounded px-2.5 text-[12px] transition-colors ${value === m.id ? "bg-accent text-white dark:text-[#0d1630]" : "text-muted hover:text-ink"}`}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
