# Privacy

DigitsNepal is built so that nothing leaves your Mac unless you deliberately choose a cloud provider.

| Data | Where it goes | Default |
|---|---|---|
| Text you translate | Your chosen provider only: a local server (Ollama, LM Studio, or a custom server on `localhost`) or a cloud API you enabled (Gemini, Claude, OpenAI, OpenRouter, Groq, Mistral, DeepSeek, Together, xAI, or a custom OpenAI-compatible endpoint) | Ollama (local) |
| API keys (one per provider) | macOS Keychain, service `org.digitsnepal.app`, account `<provider>-api-key` | not set |
| Settings | `~/Library/Application Support/org.digitsnepal.app/settings.json` | — |
| History | `~/Library/Application Support/org.digitsnepal.app/history.json` (0600) | **off** |
| Selected text from other apps | Read only when you press the shortcut; kept in memory until the next capture | — |
| Clipboard | Read only when you click *Use clipboard text* or the menu item *Translate Clipboard Text*; written only when you press *Copy* | fallback **off** |
| Audio | Not implemented in this version | — |

* **No telemetry, analytics or crash reporting.** The app makes no network requests except to the provider you selected.
* **Per-provider cloud consent.** Requests to any endpoint that is not on this Mac are blocked in Rust until you tick "I understand my text is sent to <provider>" for that specific provider. Listing models also requires consent.
* **Local means local.** Ollama and LM Studio addresses must be loopback addresses. Remote custom servers must use HTTPS and count as cloud, so they need consent.
* **No keylogging.** DigitsNepal never installs an event tap or watches keystrokes. Live Compose only processes text typed inside the DigitsNepal window.
* **No content logging.** Logs contain error codes, never translation text.
* **Private translations.** When history is on, you can tick *Private* to keep a single translation out of it.

## Revoking permissions

* Accessibility: System Settings › Privacy & Security › Accessibility, then remove DigitsNepal. Or run `tccutil reset Accessibility org.digitsnepal.app`.
* API keys: Settings › AI provider › Remove. Or delete the `org.digitsnepal.app` items in Keychain Access.
* History: History › Clear all… deletes the file.
* Everything: quit the app and delete `~/Library/Application Support/org.digitsnepal.app`.
