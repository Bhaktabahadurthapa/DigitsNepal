// Post-build checks: PWA manifest & icons, service worker, and that no
// secrets or server-side identifiers leaked into the shipped files.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const DIST = fileURLToPath(new URL("../dist/", import.meta.url));
const fail = (m) => { console.error("✗ " + m); process.exitCode = 1; };
const pass = (m) => console.log("✓ " + m);

const files = [];
(function walk(d) { for (const f of readdirSync(d)) { const p = join(d, f); statSync(p).isDirectory() ? walk(p) : files.push(p); } })(DIST);
const rel = (p) => p.slice(DIST.length);

// Manifest
const m = JSON.parse(readFileSync(join(DIST, "manifest.webmanifest"), "utf8"));
m.display === "standalone" && m.start_url === "/app/" && m.scope === "/app/" ? pass("manifest: standalone, scoped to /app/") : fail("manifest scope/display");
for (const icon of m.icons) {
  const buf = readFileSync(join(DIST, icon.src.replace(/^\/app\//, "")));
  const w = buf.readUInt32BE(16), h = buf.readUInt32BE(20), colorType = buf[25];
  const [ew, eh] = icon.sizes.split("x").map(Number);
  if (buf.toString("ascii", 1, 4) !== "PNG") fail(`${icon.src} is not a PNG`);
  else if (w !== ew || h !== eh) fail(`${icon.src} is ${w}x${h}, manifest says ${icon.sizes}`);
  else if (colorType === 6 || colorType === 4) fail(`${icon.src} has transparency (iOS shows it on black)`);
  else pass(`icon ${icon.src} ${w}x${h} opaque`);
}
if (!m.icons.some((i) => i.purpose === "maskable")) fail("no maskable icon");

// HTML
const html = readFileSync(join(DIST, "index.html"), "utf8");
for (const [re, what] of [
  [/viewport-fit=cover/, "viewport-fit=cover (safe areas)"],
  [/rel="apple-touch-icon"/, "apple-touch-icon"],
  [/rel="manifest"/, "manifest link"],
  [/apple-mobile-web-app-capable/, "iOS standalone meta"],
]) re.test(html) ? pass(what) : fail("missing " + what);

// Service worker & server config
const sw = readFileSync(join(DIST, "sw.js"), "utf8");
/url\.origin !== self\.location\.origin/.test(sw) ? pass("service worker never intercepts the API origin") : fail("sw may cache API");
const ht = readFileSync(join(DIST, ".htaccess"), "utf8");
/Content-Security-Policy/.test(ht) ? pass(".htaccess with CSP") : fail(".htaccess CSP missing");
if (!/connect-src 'self' https:\/\//.test(ht)) console.warn("! VITE_API_URL is empty: the build has no API origin in its CSP");

// Secrets scan
const patterns = [
  [/AIza[0-9A-Za-z_-]{30,}/, "Google API key"],
  [/GEMINI_API_KEY|SESSION_SECRET|ACCESS_CODE/, "server secret name"],
  [/generativelanguage\.googleapis\.com/, "direct Gemini endpoint"],
  [/sk-[A-Za-z0-9]{20,}/, "API key-like string"],
  [/v1\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}/, "device token"],
];
let leaks = 0;
for (const f of files) {
  if (/\.(png|woff2)$/.test(f)) continue;
  const s = readFileSync(f, "utf8");
  for (const [re, what] of patterns) if (re.test(s)) { fail(`${what} found in ${rel(f)}`); leaks++; }
}
if (!leaks) pass(`no secrets in ${files.length} built files`);
