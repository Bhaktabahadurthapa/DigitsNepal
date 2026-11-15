import { useEffect, useRef } from "react";
import type { Prefs } from "../storage";
import type { Appearance, Quota } from "../types";
import { Icon } from "./Icon";

interface Props {
  prefs: Prefs;
  quota: Quota | null;
  historyCount: number;
  onChange: (p: Prefs) => void;
  onClearHistory: () => void;
  onSignOut: () => void;
  onClose: () => void;
}

export function Settings({ prefs, quota, historyCount, onChange, onClearHistory, onSignOut, onClose }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (d && !d.open && typeof d.showModal === "function") d.showModal();
  }, []);
  const set = <K extends keyof Prefs>(k: K, v: Prefs[K]) => onChange({ ...prefs, [k]: v });

  return (
    <dialog ref={ref} className="sheet" aria-labelledby="settings-h" onClose={onClose} onClick={(e) => e.target === ref.current && ref.current?.close()}>
      <div className="sheet-body">
        <div className="sheet-head">
          <h2 id="settings-h">Settings</h2>
          <button className="icon-btn" onClick={() => ref.current?.close()} aria-label="Close settings"><Icon name="x" /></button>
        </div>

        <fieldset className="group">
          <legend>Appearance</legend>
          <div className="seg" role="radiogroup" aria-label="Appearance">
            {(["system", "light", "dark"] as Appearance[]).map((a) => (
              <button key={a} role="radio" aria-checked={prefs.appearance === a} onClick={() => set("appearance", a)}>
                {a === "system" ? "Auto" : a[0]!.toUpperCase() + a.slice(1)}
              </button>
            ))}
          </div>
        </fieldset>

        <label className="row">
          <span><b>Word meanings</b><small>Explain each Nepali word in the result.</small></span>
          <input type="checkbox" role="switch" checked={prefs.meanings} onChange={(e) => set("meanings", e.target.checked)} />
        </label>

        <label className="row">
          <span><b>Save history</b><small>Off by default. Stored only on this device.</small></span>
          <input type="checkbox" role="switch" checked={prefs.history} onChange={(e) => set("history", e.target.checked)} />
        </label>
        {historyCount > 0 && (
          <button className="btn btn-line wide" onClick={onClearHistory}>Delete {historyCount} saved translation{historyCount === 1 ? "" : "s"}</button>
        )}

        {quota && (
          <p className="quota">
            <span>Today</span>
            <meter min={0} max={quota.limit} value={quota.used} />
            <span>{quota.used} / {quota.limit}</span>
          </p>
        )}

        <button className="btn btn-line wide danger" onClick={onSignOut}>Sign out of this device</button>
        <p className="fine">
          Text you translate is sent through DigitsNepal's private server to Google Gemini. DigitsNepal does not store it,
          but on Gemini's free tier Google may keep and review it to improve its products. Avoid confidential text.
          {" "}<a className="link" href="/privacy">Privacy</a>
        </p>
      </div>
    </dialog>
  );
}
