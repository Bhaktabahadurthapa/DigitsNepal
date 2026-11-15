import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { ApiError, checkSession, translate } from "./api";
import { Icon, Pennant } from "./components/Icon";
import { Result } from "./components/Result";
import { Settings } from "./components/Settings";
import { SignIn } from "./components/SignIn";
import {
  addHistory, clearHistory, clearSession, loadHistory, loadPrefs, loadSession, savePrefs, saveSession,
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
const EXAMPLE = "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.";

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
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const inflight = useRef(0);

  useEffect(() => applyAppearance(prefs.appearance), [prefs.appearance]);

  useEffect(() => {
    const on = () => setOnline(true), off = () => setOnline(false);
    window.addEventListener("online", on);
    window.addEventListener("offline", off);
    return () => { window.removeEventListener("online", on); window.removeEventListener("offline", off); };
  }, []);

  const signOut = useCallback((message?: string) => {
    clearSession();
    setSession(null);
    setResult(null);
    setQuota(null);
    setSettings(false);
    setNotice(message);
  }, []);

  useEffect(() => {
    if (!session) return;
    checkSession(session.token)
      .then((s) => setQuota(s.quota))
      .catch((e) => { if (e instanceof ApiError && e.status === 401) signOut("Your session ended. Please sign in again."); });
  }, [session, signOut]);

  const updatePrefs = (p: Prefs) => {
    if (prefs.history && !p.history) { clearHistory(); setHistory([]); }
    if (!prefs.history && p.history) setHistory(loadHistory());
    setPrefs(p);
    savePrefs(p);
  };

  async function run(m: Mode = mode) {
    const value = text.trim();
    if (!session || !value || busy) return;
    const id = ++inflight.current;
    setBusy(true);
    setError(null);
    inputRef.current?.blur(); // drop the iPhone keyboard so the result is visible
    try {
      const t = await translate(session.token, value, m, prefs.meanings);
      if (id !== inflight.current) return;
      setResult(t);
      setQuota(t.quota);
      if (prefs.history) {
        setHistory(addHistory({ id: crypto.randomUUID(), at: Date.now(), text: value, mode: m, english: t.mode === "professional" && t.styled_english ? t.styled_english : t.accurate_english, nepali: t.nepali_script }));
      }
      requestAnimationFrame(() => document.getElementById("result")?.scrollIntoView?.({ behavior: "smooth", block: "start" }));
    } catch (e) {
      if (id !== inflight.current) return;
      if (e instanceof ApiError && e.status === 401) return signOut("Your session ended. Please sign in again.");
      setError(e instanceof ApiError ? { code: e.code, message: e.message } : { code: "unknown", message: "Something went wrong. Please try again." });
    } finally {
      if (id === inflight.current) setBusy(false);
    }
  }

  function pickMode(m: Mode) {
    setMode(m);
    if (result && text.trim() === lastText.current) void run(m);
  }
  const lastText = useRef("");
  useEffect(() => { if (result) lastText.current = text.trim(); }, [result]); // eslint-disable-line react-hooks/exhaustive-deps

  function onKey(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void run();
    }
  }

  async function paste() {
    try {
      const t = await navigator.clipboard.readText();
      if (t) setText(t.slice(0, MAX_CHARS));
    } catch { inputRef.current?.focus(); }
  }

  if (!session) {
    return <SignIn notice={notice} onSignedIn={(s) => { saveSession(s); setNotice(undefined); setSession(s); }} />;
  }

  const count = [...text].length;
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
            <button key={m.id} role="tab" aria-selected={mode === m.id} onClick={() => pickMode(m.id)} disabled={busy}>
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
            rows={4}
            placeholder={mode === "grammar" ? "Write English to check, e.g. He don't know the answer yesterday." : "Type Romanized Nepali, e.g. Malai thaha chaina, bholi feri herchu."}
            autoCapitalize="sentences"
            autoCorrect="off"
            spellCheck={false}
            enterKeyHint="go"
            lang={mode === "grammar" ? "en" : "ne-Latn"}
          />
          <div className="composer-bar">
            {text ? (
              <button className="mini" onClick={() => { setText(""); setResult(null); setError(null); inputRef.current?.focus(); }}><Icon name="x" size={15} /> Clear</button>
            ) : (
              <>
                <button className="mini" onClick={paste}><Icon name="paste" size={15} /> Paste</button>
                <button className="mini" onClick={() => setText(EXAMPLE)}>Try an example</button>
              </>
            )}
            <span className={"count" + (count > MAX_CHARS * 0.9 ? " warn" : "")}>{count}/{MAX_CHARS}</span>
          </div>
        </div>

        <button className="btn btn-primary go" onClick={() => run()} disabled={busy || !text.trim()}>
          {busy ? <><span className="spinner" aria-hidden="true" /> Translating…</> : mode === "grammar" ? "Check grammar" : "Translate"}
        </button>

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
