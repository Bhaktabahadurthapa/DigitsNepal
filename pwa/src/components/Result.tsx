import { useState } from "react";
import { canShare, copyText, shareText } from "../share";
import type { Translation } from "../types";
import { Icon } from "./Icon";

export function primaryText(t: Translation): string {
  return t.mode === "professional" && t.styled_english ? t.styled_english : t.accurate_english;
}

export function Result({ t, showMeanings }: { t: Translation; showMeanings: boolean }) {
  const [flash, setFlash] = useState<string | null>(null);
  const main = primaryText(t);
  const alt = t.mode === "professional" ? t.accurate_english : t.styled_english;
  const altLabel = t.mode === "professional" ? "Faithful" : "Professional";

  const notify = (m: string) => {
    setFlash(m);
    window.setTimeout(() => setFlash(null), 1600);
  };
  const copy = async (text: string) => notify((await copyText(text)) ? "Copied" : "Couldn't copy");
  const share = async () => {
    const r = await shareText(main);
    if (r === "copied") notify("Copied");
    else if (r === "failed") notify("Couldn't share");
  };

  return (
    <section className="result card" aria-live="polite" aria-label="Translation">
      <p className="kicker">{t.mode === "grammar" ? "Corrected" : t.mode === "professional" ? "Professional English" : "English"}</p>
      <p className="english" lang="en">{main}</p>

      <div className="actions">
        <button className="btn btn-primary" onClick={() => copy(main)}><Icon name="copy" size={18} /> Copy</button>
        {canShare() && <button className="btn btn-line" onClick={share}><Icon name="share" size={18} /> Share</button>}
        <span className="flash" role="status">{flash && <><Icon name="check" size={16} /> {flash}</>}</span>
      </div>

      {t.nepali_script && t.detected_language !== "english" && (
        <p className="deva source-deva" lang="ne">{t.nepali_script}</p>
      )}

      {alt && alt !== main && (
        <div className="alt">
          <div className="alt-head">
            <span className="kicker">{altLabel}</span>
            <button className="mini" onClick={() => copy(alt)} aria-label={`Copy ${altLabel.toLowerCase()} version`}><Icon name="copy" size={15} /></button>
          </div>
          <p>{alt}</p>
        </div>
      )}

      {t.warnings.length > 0 && (
        <ul className="warnings">
          {t.warnings.map((w) => <li key={w}><Icon name="alert" size={15} /> {w}</li>)}
        </ul>
      )}

      {t.grammar_notes.length > 0 && (
        <div className="block">
          <p className="kicker">What changed</p>
          <ol className="notes">
            {t.grammar_notes.map((n, i) => (
              <li key={i}>
                <p><del>{n.original}</del> <Icon name="arrow" size={14} /> <ins>{n.corrected}</ins></p>
                <p className="muted">{n.explanation}</p>
              </li>
            ))}
          </ol>
        </div>
      )}
      {t.mode === "grammar" && t.grammar_notes.length === 0 && <p className="muted small">No corrections needed.</p>}

      {showMeanings && t.word_meanings.length > 0 && (
        <div className="block">
          <p className="kicker">Word meanings</p>
          <ul className="meanings">
            {t.word_meanings.map((w, i) => (
              <li key={i} className={w.uncertain ? "uncertain" : undefined}>
                <div className="m-head">
                  <b>{w.source}</b>
                  {w.devanagari && <span className="deva" lang="ne">{w.devanagari}</span>}
                </div>
                <p className="m-mean">{w.meaning}{w.uncertain && <span className="tag">uncertain</span>}</p>
                {(w.literal || w.note) && (
                  <p className="m-note">
                    {w.literal && <>literally <i>{w.literal}</i>{w.note ? " · " : ""}</>}
                    {w.note}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {t.ambiguities.length > 0 && (
        <div className="block">
          <p className="kicker">Could also mean</p>
          <ul className="ambig">{t.ambiguities.map((a) => <li key={a}>{a}</li>)}</ul>
        </div>
      )}
    </section>
  );
}
