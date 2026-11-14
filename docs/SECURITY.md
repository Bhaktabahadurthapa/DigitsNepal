# Security policy & threat model

## Reporting a vulnerability

Please report privately via GitHub Security Advisories ("Report a vulnerability") on the repository, not in public issues. We aim to respond within 7 days.

## Threat model

| Asset / risk | Threat | Mitigation |
|---|---|---|
| **Accessibility permission** | A powerful permission: it could read or change any app's UI | Used only on an explicit shortcut press, only for the focused element's *selected* text. No background reads, no event taps. Secure input (`IsSecureEventInputEnabled`) and `AXSecureTextField` are refused. |
| **Replacing text in another app** | Overwriting the wrong text if the user moved on; data loss | Replace only if the current selection exactly equals the captured text. On failure nothing is changed and the user can copy instead. Undo checks the inserted text is still intact before restoring. |
| **Clipboard** | Silently reading secrets or overwriting clipboard contents | Read only on explicit, labelled user actions; written only on *Copy*. The fallback is off by default. Clipboard contents are never shown in notifications. |
| **API keys** | Leaking through files, logs, the webview, or URLs | Stored in the Keychain; the webview can only *set*, *delete* or ask *exists?*. Sent only in request headers (`Authorization: Bearer`, `x-api-key`, `x-goog-api-key`), never in a URL. Non-editable presets ignore endpoint overrides, so a key for OpenAI can only ever go to api.openai.com. Error messages strip URLs. |
| **Model responses** | Malicious or malformed output (prompt injection from translated text, oversized fields, fake "original text") | Strict validation: JSON-only, required fields, length caps, `original_text` replaced with the real input, unknown fields ignored. Rendered as text only (React escaping, no `dangerouslySetInnerHTML`). |
| **Malicious app text** | Selected text that contains instructions ("ignore previous instructions…") | Source is fenced as data in `<source>`; closing tags are neutralised; the system prompt says to treat it as data. Output still goes through validation, and a human always reviews it before Replace. |
| **Webview compromise** | XSS reaching privileged commands | Strict CSP (`script-src 'self'`, no remote content). Capabilities grant no plugin access to the webview: clipboard, shortcuts, network and AX run only through validated app commands. |
| **Network exfiltration** | "Local" provider pointed at a remote host | Local presets must use loopback; remote custom servers must use HTTPS; credentials in URLs are rejected; every non-loopback endpoint needs per-provider consent. |
| **Shell injection** | Commands built from user input | The only process spawned is `/usr/bin/open` with a constant URL. |
| **Local data at rest** | Other users reading history | Files written atomically with mode 0600 in the user's Application Support folder. History is opt-in. |

## Notes

* `macOSPrivateApi` is enabled in `tauri.conf.json` so windows can be transparent (Overlay opacity). That rules out Mac App Store distribution, but it doesn't affect direct downloads.

## Known limitations

* Unsigned builds can't be verified by Gatekeeper; build from source or check the release checksum.
* AX access works differently in each app. Some apps (certain Electron or Java apps, terminal emulators) don't expose selected text, or ignore `AXSelectedText` writes. See the compatibility table in the README.
