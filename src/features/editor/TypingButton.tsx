import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

/** The small button shown beside a text field in another app while the user
 *  types Nepali. Clicking it translates that field: either in place (one-step
 *  mode) or via the preview with Replace. */
export function TypingButton() {
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const un = listen<boolean>("bubble:busy", (e) => setBusy(e.payload));
    return () => void un.then((f) => f());
  }, []);
  return (
    <button
      type="button"
      disabled={busy}
      title={busy ? "Translating…" : "Translate to English (DigitsNepal)"}
      aria-label="Translate this text to English with DigitsNepal"
      aria-busy={busy}
      onClick={() => void invoke("typing_button_clicked")}
      className="deva flex h-[30px] w-[30px] items-center justify-center rounded-full bg-accent text-[15px] font-semibold text-on-accent shadow-[0_2px_8px_rgba(0,0,0,0.25)] transition-transform hover:scale-110 active:scale-95 disabled:hover:scale-100"
      style={{ margin: 2 }}
    >
      {busy ? (
        <span className="h-[14px] w-[14px] animate-spin rounded-full border-2 border-current border-t-transparent" />
      ) : (
        "अ"
      )}
    </button>
  );
}
