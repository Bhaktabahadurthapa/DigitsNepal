# digitsnepal.com

Dynamic PHP site (Hostinger, PHP 8) in the "interlinear manuscript" design. No database, no third-party requests from visitors' browsers.

| Path | What it is |
|---|---|
| `index.php` | Front controller; whitelisted pages only |
| `.htaccess` | HTTPS, clean URLs (`/dictionary`, `/download`, `/changelog`, `/privacy`), 404s, blocks `inc/` |
| `inc/layout.php` | Shared head, masthead (active nav), footer |
| `inc/release.php` | Latest release + changelog from the GitHub API, fetched server-side, cached 1 h outside the web root (`../dn-cache`), with fallback |
| `inc/markdown.php` | Escape-first Markdown for release notes |
| `inc/pages/*.php` | Home, Dictionary, Download, Changelog, Privacy, 404 |
| `data/lexicon.json` | The app's offline lexicon (`python3 scripts/export-lexicon.py`) |
| `dictionary.js` | Instant search, category filter, sentence glosser (runs in the browser) |
| `site.js` / `site.css` | Interlinear demo, reveals, styles |

New GitHub releases appear on /download and /changelog automatically within an hour.

## Deploy

```bash
cd website && zip -r -X ../digitsnepal-site.zip . -x README.md
```

Upload the zip to `public_html` (hPanel › File Manager), open the File Manager **Terminal**, then:

```bash
unzip -o -q digitsnepal-site.zip && rm digitsnepal-site.zip
```

To show a just-published release immediately: `rm -f ../dn-cache/releases.json`.
