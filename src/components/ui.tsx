import { useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import { api } from "../providers/api";
import type { AppError } from "../types/translation";

type Variant = "primary" | "secondary" | "ghost" | "danger";

export function Button({ variant = "secondary", className = "", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  const base = "inline-flex items-center justify-center gap-1.5 rounded-lg px-3 h-8 text-[12.5px] font-medium transition-colors duration-150 disabled:opacity-45 disabled:cursor-not-allowed";
  const styles: Record<Variant, string> = {
    primary: "bg-accent text-on-accent shadow-card hover:bg-accent-hover",
    secondary: "bg-surface border border-line text-ink shadow-card hover:bg-surface-2",
    ghost: "text-muted hover:text-ink hover:bg-surface-2",
    danger: "bg-surface border border-line text-error-text hover:bg-error-bg",
  };
  return <button type="button" className={`${base} ${styles[variant]} ${className}`} {...rest} />;
}

export function CopyButton({ text, label = "Copy", disabled }: { text: string | null | undefined; label?: string; disabled?: boolean }) {
  const [state, setState] = useState<"idle" | "done" | "error">("idle");
  return (
    <Button
      variant="ghost"
      disabled={disabled || !text}
      aria-label={`${label} to clipboard`}
      onClick={async () => {
        if (!text) return;
        try {
          await api.copyText(text);
          setState("done");
        } catch {
          setState("error");
        }
        setTimeout(() => setState("idle"), 1400);
      }}
    >
      {state === "done" ? "Copied" : state === "error" ? "Copy failed" : label}
    </Button>
  );
}

export function ErrorBanner({ error, onRetry, title = "Translation unavailable", note = "Your original text has not been changed." }: { error: AppError | null; onRetry?: () => void; title?: string; note?: string | null }) {
  if (!error) return null;
  return (
    <div role="alert" className="fade-in flex items-start gap-3 rounded-lg border border-[color-mix(in_srgb,var(--error-text)_20%,transparent)] bg-error-bg px-3 py-2.5 text-error-text">
      <div className="flex-1">
        <div className="font-medium">{title}</div>
        <div className="opacity-90">{error.message}</div>
        {note && <div className="mt-0.5 text-[11px] opacity-70">{note}</div>}
      </div>
      {error.retryable && onRetry && (
        <Button variant="secondary" onClick={onRetry}>
          Retry
        </Button>
      )}
    </div>
  );
}

export function Notice({ children, tone = "warn" }: { children: ReactNode; tone?: "warn" | "info" }) {
  const cls = tone === "warn" ? "bg-warn-bg text-warn-text" : "bg-accent-soft text-ink";
  return <div className={`rounded-lg px-3 py-2 text-[12.5px] leading-relaxed ${cls}`}>{children}</div>;
}

export function Panel({ title, actions, children, className = "" }: { title: ReactNode; actions?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`flex min-h-0 flex-col rounded-xl border border-line bg-surface shadow-card ${className}`}>
      <header className="flex h-10 shrink-0 items-center justify-between border-b border-line px-4">
        <h2 className="text-[12px] font-semibold text-ink">{title}</h2>
        <div className="flex items-center gap-1">{actions}</div>
      </header>
      <div className="min-h-0 flex-1 overflow-auto">{children}</div>
    </section>
  );
}

export function Toggle({ checked, onChange, label, description, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; description?: ReactNode; disabled?: boolean }) {
  return (
    <label className={`flex items-start justify-between gap-6 py-2.5 ${disabled ? "opacity-50" : ""}`}>
      <span>
        <span className="block font-medium">{label}</span>
        {description && <span className="mt-0.5 block text-[12px] text-muted">{description}</span>}
      </span>
      <input
        type="checkbox"
        role="switch"
        className="mt-0.5 h-4 w-4 accent-[var(--accent)]"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
    </label>
  );
}

export function Spinner({ label = "Translating…" }: { label?: string }) {
  return (
    <div role="status" className="flex items-center gap-2 text-muted">
      <span className="h-3 w-3 animate-spin rounded-full border-2 border-line border-t-accent" aria-hidden />
      {label}
    </div>
  );
}
