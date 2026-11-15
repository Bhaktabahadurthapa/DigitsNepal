import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { ApiError, checkSession, translate } from "./api";
import { Icon, Pennant } from "./components/Icon";
import { primaryText, Result } from "./components/Result";
import { Settings } from "./components/Settings";
import { SignIn } from "./components/SignIn";
import { copyText } from "./share";
import {
  addHistory, clearHistory, clearSession, loadHistory, loadPrefs, loadSession, requestPersistence, savePrefs, saveSession,
  type Prefs, type Session,
} from "./storage";
import { applyAppearance } from "./theme";
import type { HistoryItem, Mode, Quota, Translation } from "./types";

const MODES: { id: Mode; label: string; hint: string }[] = [
  { id: "accurate", label: "Accurate", hint: "Faithful translation" },
  { id: "professional", label: "Professional", hint: "Workplace English" },
  { id: "grammar", label: "Grammar", hint: "Fix & explain" },
];
const MAX_CHARS = 2000;
/** Pause after the last keystroke before translating live. */
export const LIVE_DELAY_MS = 900;
const EXAMPLE = "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.";

/** Live translation only once there is a real word to work with. */
const worthTranslating = (s: string) => s.replace(/[^\p{L}\p{N}]/gu, "").length >= 2;

export default function App() {
  const [session, setSession] = useState<Session | null>(loadSession);
  const [notice, setNotice] = useState<string | undefined>();
  const [prefs, setPrefs] = useState<Prefs>(loadPrefs);
  const [quota, setQuota] = useState<Quota | null>(null);
  const [mode, setMode] = useState<Mode>("accurate");
  const [text, setText] = useState("");
  const [result, setResult] = useState<Translation | null>(null);
  const [error, setError] = useState<{ code: string; message: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [settings, setSettings] = useState(false);
  const [history, setHistory] = useState<HistoryItem[]>(() => (loadPrefs().history ? loadHistory() : []));
  const [online, setOnline] = useState(() => navigator.onLine);
  const [copied, setCopied] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const inflight = useRef<AbortController | null>(null);
  const done = useRef<{ text: string; mode: Mode } | null>(null); // last translated input

  useEffect(() => applyAppearance(prefs.appearance), [prefs.appearance]);
  useEffect(() => requestPersistence(), []);

  useEffect(() => {
    const on = () => setOnline(true), off = () => setOnline(false);
    window.addEventListener("online", on);
    window.addEventListener("offline", off);
    return () => { window.removeEventListener("online", on); window.removeEventListener("offline", off); };
  }, []);

  const signOut = useCallback((message?: string) => {
    inflight.current?.abort();
    clearSession();
    setSession(null);
    setResult(null);
    setQuota(null);
    setSettings(false);
    setNotice(message);
  }, []);

  // On open: confirm the sign-in, refresh the quota, and pick up a renewed token
  // so a device that keeps using the app never has to enter the code again.
  const token = session?.token;
  useEffect(() => {
    if (!token) return;
    checkSession(token)
      .then((s) => {
        setQuota(s.quota);
        if (s.token && s.token !== token) {
          const renewed = { token: s.token, expiresAt: s.expiresAt };
          saveSession(renewed);
          setSession(renewed);
        }
      })
      .catch((e) => { if (e instanceof ApiError && e.status === 401) signOut("Your sign-in expired. Please enter your access code again."); });
  }, [token, signOut]);

  const updatePrefs = (p: Prefs) => {
    if (prefs.history && !p.history) { clearHistory(); setHistory([]); }
    if (!prefs.history && p.history) setHistory(loadHistory());
    setPrefs(p);
    savePrefs(p);
  };

  const run = useCallback(async (value: string, m: Mode, auto: boolean) => {
    if (!session || !worthTranslating(value)) return;
    inflight.current?.abort();
    const ctl = new AbortController();
    inflight.current = ctl;
    setBusy(true);
    if (!auto) {
      setError(null);
      inputRef.current?.blur(); // drop the keyboard so the full result is visible
    }
    try {
      const t = await translate(session.token, value, m, prefs.meanings, ctl.signal);
      if (ctl.signal.aborted) return;
      done.current = { text: value, mode: m };
      setResult(t);
      setQuota(t.quota);
      setError(null);
      if (prefs.history) {
        setHistory(addHistory({ id: crypto.randomUUID(), at: Date.now(), text: value, mode: m, english: primaryText(t), nepali: t.nepali_script }));
      }
      if (!auto) requestAnimationFrame(() => document.getElementById("result")?.scrollIntoView?.({ behavior: "smooth", block: "start" }));
    } catch (e) {
      if (ctl.signal.aborted || (e instanceof ApiError && e.code === "aborted")) return;
      if (e instanceof ApiError && e.status === 401) return signOut("Your sign-in expired. Please enter your access code again.");
      // While typing, a momentary rate limit isn't worth an alarm: the next pause retries.
      if (auto && e instanceof ApiError && e.code === "slow_down") return;
      setError(e instanceof ApiError ? { code: e.code, message: e.message } : { code: "unknown", message: "Something went wrong. Please try again." });
    } finally {
      if (inflight.current === ctl) { inflight.current = null; setBusy(false); }
    }
  }, [session, prefs.meanings, prefs.history, signOut]);

  // Live translation: translate after a short pause in typing, cancelling any older request.
  useEffect(() => {
    if (!prefs.live || !session) return;
    const value = text.trim();
    if (!worthTranslating(value) || !online) return;
    if (done.current && done.current.text === value && done.current.mode === mode) return;
    const id = window.setTimeout(() => void run(value, mode, true), LIVE_DELAY_MS);
    return () => window.clearTimeout(id);
  }, [text, mode, prefs.live, session, online, run]);

  function onKey(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void run(text.trim(), mode, false);
    }
  }

  function pickMode(m: Mode) {
    setMode(m);
    if (!prefs.live && result && text.trim()) void run(text.trim(), m, false);
  }

  async function paste() {
    try {
      const t = await navigator.clipboard.readText();
      if (t) setText(t.slice(0, MAX_CHARS));
    } catch { inputRef.current?.focus(); }
  }

  async function copyLive() {
    if (!result) return;
    if (await copyText(primaryText(result))) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    }
  }

  function clear() {
    inflight.current?.abort();
    done.current = null;
    setText("");
    setResult(null);
    setError(null);
    inputRef.current?.focus();
  }

  if (!session) {
    return <SignIn notice={notice} onSignedIn={(s) => { saveSession(s); requestPersistence(); setNotice(undefined); setSession(s); }} />;
  }

  const count = [...text].length;
  const stale = !!result && (done.current?.text !== text.trim() || done.current?.mode !== mode);
  const showLive = prefs.live && !!text.trim();
  return (
    <div className="app">
      <header className="top">
        <div className="brand"><Pennant size={24} /><span>DigitsNepal</span></div>
        <button className="icon-btn" onClick={() => setSettings(true)} aria-label="Settings"><Icon name="gear" size={22} /></button>
      </header>

      <main className="main">
        {!online && <p className="banner" role="status"><Icon name="wifi" size={16} /> You're offline. Translation needs an internet connection.</p>}

        <div className="modes" role="tablist" aria-label="Translation mode">
          {MODES.map((m) => (
            <button key={m.id} role="tab" aria-selected={mode === m.id} onClick={() => pickMode(m.id)}>
              <b>{m.label}</b><small>{m.hint}</small>
            </button>
          ))}
        </div>

        <div className="card composer">
          <label htmlFor="src" className="sr-only">Text to translate</label>
          <textarea
            id="src"
            ref={inputRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={onKey}
            maxLength={MAX_CHARS}
            rows={3}
            placeholder={mode === "grammar" ? "Write English to check, e.g. He don't know the answer yesterday." : "Type Romanized Nepali, e.g. Malai thaha chaina, bholi feri herchu."}
            autoCapitalize="sentences"
            autoCorrect="off"
            spellCheck={false}
            enterKeyHint="go"
            lang={mode === "grammar" ? "en" : "ne-Latn"}
          />

          {showLive && (
            <div className={"live" + (stale || busy ? " pending" : "")} aria-live="polite">
              <div className="live-head">
                <span className="kicker"><span className={"dot" + (busy ? " on" : "")} aria-hidden="true" /> {busy ? "Translating…" : "Live English"}</span>
                {result && <button className="mini" onClick={copyLive} aria-label="Copy English">{copied ? <><Icon name="check" size={14} /> Copied</> : <><Icon name="copy" size={14} /> Copy</>}</button>}
              </div>
              <p className="live-text" lang="en">{result ? primaryText(result) : <span className="muted">Keep typing… the English appears here.</span>}</p>
            </div>
          )}

          <div className="composer-bar">
            {text ? (
              <button className="mini" onClick={clear}><Icon name="x" size={15} /> Clear</button>
            ) : (
              <>
                <button className="mini" onClick={paste}><Icon name="paste" size={15} /> Paste</button>
                <button className="mini" onClick={() => setText(EXAMPLE)}>Try an example</button>
              </>
            )}
            <span className={"count" + (count > MAX_CHARS * 0.9 ? " warn" : "")}>{count}/{MAX_CHARS}</span>
          </div>
        </div>

        {!prefs.live && (
          <button className="btn btn-primary go" onClick={() => run(text.trim(), mode, false)} disabled={busy || !text.trim()}>
            {busy ? <><span className="spinner" aria-hidden="true" /> Translating…</> : mode === "grammar" ? "Check grammar" : "Translate"}
          </button>
        )}

        {error && (
          <p className={"error-card" + (error.code === "offline" ? " offline" : "")} role="alert">
            <Icon name={error.code === "offline" ? "wifi" : "alert"} size={18} /> {error.message}
          </p>
        )}

        <div id="result">{result && <Result t={result} showMeanings={prefs.meanings} />}</div>

        {prefs.history && history.length > 0 && (
          <section className="history" aria-labelledby="hist-h">
            <h2 id="hist-h" className="kicker"><Icon name="clock" size={14} /> Recent</h2>
            <ul>
              {history.slice(0, 15).map((h) => (
                <li key={h.id}>
                  <button onClick={() => { setText(h.text); setMode(h.mode); window.scrollTo({ top: 0, behavior: "smooth" }); }}>
                    <span className="h-src">{h.text}</span>
                    <span className="h-en">{h.english}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}

        <p className="foot fine">Translations by <b>DigitsNepalAI</b>. Check important text before you send it.</p>
      </main>

      {settings && (
        <Settings
          prefs={prefs}
          quota={quota}
          historyCount={history.length}
          onChange={updatePrefs}
          onClearHistory={() => { clearHistory(); setHistory([]); }}
          onSignOut={() => signOut()}
          onClose={() => setSettings(false)}
        />
      )}
    </div>
  );
}
