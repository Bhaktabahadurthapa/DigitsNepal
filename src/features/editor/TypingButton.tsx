import { invoke } from "@tauri-apps/api/core";

/** The small button shown beside a text field in another app while the user
 *  types Nepali. Clicking it translates that field and offers Replace. */
export function TypingButton() {
  return (
    <button
      type="button"
      title="Translate to English (DigitsNepal)"
      aria-label="Translate this text to English with DigitsNepal"
      onClick={() => void invoke("typing_button_clicked")}
      className="deva flex h-[30px] w-[30px] items-center justify-center rounded-full bg-accent text-[15px] font-semibold text-on-accent shadow-[0_2px_8px_rgba(0,0,0,0.25)] transition-transform hover:scale-110 active:scale-95"
      style={{ margin: 2 }}
    >
      अ
    </button>
  );
}
