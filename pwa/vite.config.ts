/// <reference types="vitest/config" />
import { createHash } from "node:crypto";
import { defineConfig, loadEnv, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

const BASE = "/app/";

/**
 * Emits sw.js (precache list + content hash, so every deploy updates the cache)
 * and an .htaccess for Hostinger with a strict CSP that only allows the API origin.
 */
function pwaFiles(apiUrl: string): Plugin {
  return {
    name: "digitsnepal-pwa-files",
    apply: "build",
    generateBundle(_opts, bundle) {
      const files = Object.keys(bundle).filter((f) => !f.endsWith(".map"));
      const shell = ["", "index.html", "manifest.webmanifest", "icons/icon-192.png", "icons/icon-512.png", "icons/apple-touch-icon.png", "icons/favicon-64.png"];
      const precache = [...new Set([...shell, ...files.filter((f) => f.startsWith("assets/"))])].map((f) => BASE + f);
      const version = createHash("sha256").update(precache.join("|")).digest("hex").slice(0, 12);
      this.emitFile({ type: "asset", fileName: "sw.js", source: swSource(version, precache) });

      const api = apiUrl ? new URL(apiUrl).origin : "";
      const csp = [
        "default-src 'self'",
        `connect-src 'self'${api ? " " + api : ""}`,
        "script-src 'self'",
        "style-src 'self'",
        "img-src 'self' data:",
        "font-src 'self'",
        "manifest-src 'self'",
        "worker-src 'self'",
        "object-src 'none'",
        "base-uri 'self'",
        "form-action 'self'",
        "frame-ancestors 'none'",
      ].join("; ");
      this.emitFile({ type: "asset", fileName: ".htaccess", source: htaccess(csp) });
    },
  };
}

function swSource(version: string, precache: string[]): string {
  return `// DigitsNepal PWA service worker. Caches the app shell only.
// Translation requests go to a different origin and are never cached.
const CACHE = "dn-app-${version}";
const PRECACHE = ${JSON.stringify(precache)};

self.addEventListener("install", (e) => {
  e.waitUntil(caches.open(CACHE).then((c) => c.addAll(PRECACHE)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("dn-app-") && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (e) => {
  const req = e.request;
  const url = new URL(req.url);
  if (req.method !== "GET" || url.origin !== self.location.origin || !url.pathname.startsWith("${BASE}")) return;
  if (req.mode === "navigate") {
    // Network first so updates arrive; fall back to the cached shell offline.
    e.respondWith(fetch(req).catch(() => caches.match("${BASE}index.html")));
    return;
  }
  e.respondWith(caches.match(req).then((hit) => hit || fetch(req)));
});
`;
}

function htaccess(csp: string): string {
  return `# DigitsNepal iPhone web app (/app). Self-contained: parent rewrite rules do not apply here.
DirectoryIndex index.html
Options -Indexes
RewriteEngine On

RewriteCond %{HTTPS} !=on
RewriteRule ^ https://%{HTTP_HOST}%{REQUEST_URI} [L,R=301]

# Single-page app: unknown paths load the app shell.
RewriteCond %{REQUEST_FILENAME} !-f
RewriteCond %{REQUEST_FILENAME} !-d
RewriteRule ^ index.html [L]

AddType application/manifest+json .webmanifest

<IfModule mod_headers.c>
  Header always set Content-Security-Policy "${csp}"
  Header always set X-Content-Type-Options "nosniff"
  Header always set Referrer-Policy "no-referrer"
  Header always set X-Frame-Options "DENY"
  Header always set Permissions-Policy "camera=(), microphone=(), geolocation=()"
  <FilesMatch "^(index\\.html|sw\\.js|manifest\\.webmanifest)$">
    Header set Cache-Control "no-cache"
  </FilesMatch>
  <FilesMatch "\\.(js|css|woff2)$">
    Header set Cache-Control "public, max-age=31536000, immutable"
  </FilesMatch>
</IfModule>
<Files "sw.js">
  Header set Service-Worker-Allowed "/app/"
  Header set Cache-Control "no-cache"
</Files>
`;
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");
  return {
    base: BASE,
    plugins: [react(), pwaFiles(env.VITE_API_URL ?? "")],
    build: { target: "safari15", sourcemap: false, assetsInlineLimit: 0 },
    test: { environment: "jsdom", globals: true, setupFiles: ["./test/setup.ts"], env: { VITE_API_URL: "https://api.test" } },
  };
});
