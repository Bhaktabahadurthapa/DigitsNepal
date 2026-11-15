# DigitsNepal for iPhone (web app)

A private, installable web app at **https://digitsnepal.com/app**. It needs no App Store, no
Apple Developer membership and no paid services.

```
iPhone (Safari / Home Screen app)
   │  HTTPS, Bearer device token
   ▼
Cloudflare Worker  digitsnepal-api  (free plan)
   │  access-code sign-in · daily quota · burst limit · size limits
   │  GEMINI_API_KEY lives here only (Worker secret)
   ▼
Google Gemini API (free tier)
```

| Part | Where | Source |
|---|---|---|
| Web app (static PWA) | Hostinger, `public_html/app/` | `pwa/` |
| Private API | Cloudflare Workers, `digitsnepal-api` | `worker/` |
| Daily counters | Cloudflare Workers KV (`LIMITS`) | — |

## Security model

- **No secrets in the web app.** The built files contain only the public Worker URL. `npm run build`
  scans every built file for keys, tokens and secret names, and fails if it finds any.
- **Personal access.** You sign in once per device with an access code (a Worker secret, at least
  12 characters). The Worker returns a signed device token (HMAC-SHA256, valid 30 days).
  Changing `SESSION_SECRET` signs every device out.
- **Brute-force protection.** 5 wrong codes per network per 15 minutes, and 40 per day in total,
  before sign-in locks. IP addresses are stored only as keyed hashes that expire.
- **Origin lock.** The API answers only `https://digitsnepal.com`. Requests from other sites, or with
  no `Origin` header, get `403`.
- **Abuse limits.** 150 translations per UTC day (all devices together), 12 per minute per device,
  2,000 characters per request, and 16 KB per request body.
- **No content logging.** Worker observability is off, and errors log only an error class, never your text.
- **No caching of translations.** API responses are `Cache-Control: no-store`, and the service worker
  caches only the app's own files.
- **Strict CSP** on `/app`: scripts, styles and fonts load only from digitsnepal.com, and the app can
  connect only to the Worker.

**Privacy note.** On Gemini's free tier, Google may keep and review submitted text to improve its
products. Don't translate confidential text through the iPhone app. The Mac app can run fully
locally with Ollama instead.

## One-time setup (manual steps)

Run these in a terminal from the `worker/` folder. `npx wrangler login` is only needed once per computer.

1. **Create a Gemini API key.** Go to <https://aistudio.google.com/apikey> and choose **Create API key**.
   Keep the project on the free tier and don't enable billing. Under **API restrictions**, restrict the
   key to the *Generative Language API* (Google stops accepting unrestricted keys).

2. **Store it in the Worker.** Paste the key when prompted; it is never written to disk or the repo:
   ```bash
   npx wrangler secret put GEMINI_API_KEY
   ```

3. **Choose your access code.** Use at least 12 characters, ideally a passphrase of four or more
   random words. This is what you type on the iPhone.
   ```bash
   npx wrangler secret put ACCESS_CODE
   ```

4. `SESSION_SECRET` is already set to a random value. To sign out every device, set a new one:
   ```bash
   openssl rand -base64 48 | tr -d '\n' | npx wrangler secret put SESSION_SECRET
   ```

Secrets take effect within a few seconds; no redeploy is needed.

## Install on iPhone

1. Open **https://digitsnepal.com/app** in **Safari**.
2. Sign in with your access code. iCloud Keychain offers to save it.
3. Tap **Share**, then **Add to Home Screen**, then **Add**.

The app opens full screen with its own icon. Sign-in lasts 30 days on that device.

## Changing limits

Edit `[vars]` in `worker/wrangler.toml`, then run `npx wrangler deploy`:

| Variable | Default | Meaning |
|---|---|---|
| `DAILY_LIMIT` | 150 | Translations per UTC day, all devices |
| `MAX_CHARS` | 2000 | Characters per request |
| `SESSION_DAYS` | 30 | Device sign-in lifetime |
| `GEMINI_MODEL` | *(empty)* | Empty picks the newest available Flash model automatically, falling back to the next one if a model is rate-limited. Set a model ID to pin one. |
| `ALLOWED_ORIGINS` | digitsnepal.com | Comma-separated origins allowed to call the API |

Free-tier headroom: Workers allows 100,000 requests a day, and KV allows 1,000 writes a day
(each translation uses one write). Gemini's free-tier quotas are set by Google and change often;
when they run out, the app says so and asks you to try later.

## Develop and deploy

```bash
# API
cd worker && npm install && npm test && npx wrangler deploy

# Web app (writes dist/, then checks the manifest, icons, CSP and scans for secrets)
cd pwa && npm install && npm test && npm run build
```

To deploy the web app, upload the contents of `pwa/dist/` (including `.htaccess`) to
`public_html/app/` on Hostinger. The app has its own `.htaccess`, so the website's routes are untouched.
