import { useState, type FormEvent } from "react";
import { ApiError, signIn } from "../api";
import type { Session } from "../storage";
import { Icon, Pennant } from "./Icon";

export function SignIn({ onSignedIn, notice }: { onSignedIn: (s: Session) => void; notice?: string }) {
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!code.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      onSignedIn(await signIn(code.trim()));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Sign-in failed. Please try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="signin">
      <div className="signin-mark"><Pennant size={54} /></div>
      <h1>DigitsNepal</h1>
      <p className="deva" lang="ne">हरेक नेपाली शब्दको अर्थ हुन्छ।</p>
      <p className="muted">Every Nepali word has meaning.</p>
      <form onSubmit={submit} className="card signin-card" method="post" action="#" autoComplete="on">
        <label htmlFor="code" className="label"><Icon name="lock" size={16} /> Private access</label>
        {/* A real (visually hidden) username field lets iCloud Keychain save and fill the code like a password. */}
        <input className="sr-only" type="text" name="username" autoComplete="username" defaultValue="DigitsNepal" tabIndex={-1} aria-hidden="true" />
        <input
          id="code"
          name="password"
          type="password"
          autoComplete="current-password"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          enterKeyHint="go"
          placeholder="Access code"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          aria-invalid={!!error}
          aria-describedby={error ? "code-err" : undefined}
        />
        {(error || notice) && <p id="code-err" className="error" role="alert">{error ?? notice}</p>}
        <button className="btn btn-primary" type="submit" disabled={busy || !code.trim()}>
          {busy ? "Checking…" : "Sign in"}
        </button>
        <p className="fine">This translator is private. Sign in once on this iPhone and it remembers you. Let Safari save the code to Passwords when it asks.</p>
      </form>
      <a className="fine link" href="/">digitsnepal.com</a>
    </main>
  );
}
