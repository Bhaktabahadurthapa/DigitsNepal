<?php
/**
 * Release data from GitHub, fetched server-side and cached, so the site
 * updates itself when a new version is published — and visitors' browsers
 * never contact GitHub just to view a page.
 */
declare(strict_types=1);

const GH_REPO = 'Bhaktabahadurthapa/DigitsNepal';
const RELEASE_TTL = 3600;          // refresh at most once an hour
const RELEASE_RETRY_TTL = 300;     // after a failure, wait 5 minutes

/** Used only if GitHub has never been reachable from this server. */
const RELEASE_FALLBACK = [
    'tag' => 'v0.1.0',
    'name' => 'DigitsNepal 0.1.0',
    'date' => '2026-11-14T22:57:00Z',
    'url' => 'https://github.com/Bhaktabahadurthapa/DigitsNepal/releases/tag/v0.1.0',
    'notes' => '',
    'prerelease' => false,
    'dmg' => [
        'name' => 'DigitsNepal-0.1.0-AppleSilicon.dmg',
        'size' => 2774569,
        'url' => 'https://github.com/Bhaktabahadurthapa/DigitsNepal/releases/download/v0.1.0/DigitsNepal-0.1.0-AppleSilicon.dmg',
    ],
    'sha256_url' => 'https://github.com/Bhaktabahadurthapa/DigitsNepal/releases/download/v0.1.0/DigitsNepal-0.1.0-AppleSilicon.dmg.sha256',
    'sha256' => '77697fae18ae94e58485a75cd818275bb3ba5fe98b1669be805252ec06d43357',
    'downloads' => 0,
];

/** Cache lives outside the web root when possible. */
function cache_dir(): string
{
    $candidates = [];
    if (!empty($_SERVER['DOCUMENT_ROOT'])) {
        $candidates[] = dirname(rtrim($_SERVER['DOCUMENT_ROOT'], '/')) . '/dn-cache';
    }
    $candidates[] = sys_get_temp_dir() . '/dn-cache';
    foreach ($candidates as $dir) {
        if (is_dir($dir) || @mkdir($dir, 0750, true)) {
            if (is_writable($dir)) {
                return $dir;
            }
        }
    }
    return sys_get_temp_dir();
}

function http_get(string $url, bool $json = true): ?string
{
    $headers = ['User-Agent: digitsnepal.com-site', $json ? 'Accept: application/vnd.github+json' : 'Accept: */*'];
    if (function_exists('curl_init')) {
        $ch = curl_init($url);
        curl_setopt_array($ch, [
            CURLOPT_RETURNTRANSFER => true,
            CURLOPT_FOLLOWLOCATION => true,
            CURLOPT_MAXREDIRS => 5,
            CURLOPT_CONNECTTIMEOUT => 4,
            CURLOPT_TIMEOUT => 6,
            CURLOPT_HTTPHEADER => $headers,
        ]);
        $body = curl_exec($ch);
        $code = (int) curl_getinfo($ch, CURLINFO_RESPONSE_CODE);
        curl_close($ch);
        return ($body !== false && $code === 200) ? (string) $body : null;
    }
    $ctx = stream_context_create(['http' => ['header' => implode("\r\n", $headers), 'timeout' => 6, 'follow_location' => 1]]);
    $body = @file_get_contents($url, false, $ctx);
    return $body === false ? null : $body;
}

function cache_read(string $name, int $ttl): array
{
    $file = cache_dir() . '/' . $name;
    if (!is_file($file)) {
        return ['data' => null, 'fresh' => false];
    }
    $data = json_decode((string) file_get_contents($file), true);
    return ['data' => $data, 'fresh' => (time() - (int) filemtime($file)) < $ttl];
}

function cache_write(string $name, $data): void
{
    $file = cache_dir() . '/' . $name;
    $tmp = $file . '.' . getmypid() . '.tmp';
    if (@file_put_contents($tmp, json_encode($data, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE)) !== false) {
        @rename($tmp, $file);
    }
}

function normalise_release(array $r): array
{
    $dmg = null;
    $shaUrl = null;
    $downloads = 0;
    foreach ($r['assets'] ?? [] as $a) {
        $downloads += (int) ($a['download_count'] ?? 0);
        $name = (string) ($a['name'] ?? '');
        if ($shaUrl === null && preg_match('/\.dmg\.sha256$/i', $name)) {
            $shaUrl = (string) $a['browser_download_url'];
        } elseif (preg_match('/\.dmg$/i', $name)) {
            $isArm = (bool) preg_match('/apple ?silicon|aarch64|arm64/i', $name);
            if ($dmg === null || ($isArm && empty($dmg['arm']))) {
                $dmg = ['name' => $name, 'size' => (int) ($a['size'] ?? 0), 'url' => (string) $a['browser_download_url'], 'arm' => $isArm];
            }
        }
    }
    return [
        'tag' => (string) ($r['tag_name'] ?? ''),
        'name' => (string) (($r['name'] ?? '') ?: ($r['tag_name'] ?? '')),
        'date' => (string) ($r['published_at'] ?? ''),
        'url' => (string) ($r['html_url'] ?? ''),
        'notes' => (string) ($r['body'] ?? ''),
        'prerelease' => !empty($r['prerelease']),
        'dmg' => $dmg,
        'sha256_url' => $shaUrl,
        'sha256' => null,
        'downloads' => $downloads,
    ];
}

/** All published releases, newest first. */
function all_releases(): array
{
    static $memo = null;
    if ($memo !== null) {
        return $memo;
    }
    $cached = cache_read('releases.json', RELEASE_TTL);
    if ($cached['fresh'] && is_array($cached['data'])) {
        return $memo = $cached['data'];
    }
    $body = http_get('https://api.github.com/repos/' . GH_REPO . '/releases?per_page=30');
    $decoded = $body !== null ? json_decode($body, true) : null;
    if (is_array($decoded)) {
        $list = [];
        foreach ($decoded as $r) {
            if (is_array($r) && empty($r['draft'])) {
                $list[] = normalise_release($r);
            }
        }
        if ($list) {
            cache_write('releases.json', $list);
            return $memo = $list;
        }
    }
    // GitHub unreachable or rate-limited: serve the last good copy, and
    // touch it so we don't retry on every page view.
    if (is_array($cached['data']) && $cached['data']) {
        @touch(cache_dir() . '/releases.json', time() - RELEASE_TTL + RELEASE_RETRY_TTL);
        return $memo = $cached['data'];
    }
    return $memo = [RELEASE_FALLBACK];
}

function latest_release(): array
{
    foreach (all_releases() as $r) {
        if (empty($r['prerelease']) && !empty($r['dmg'])) {
            return $r;
        }
    }
    return RELEASE_FALLBACK;
}

/** SHA-256 for a release's DMG, read from its published .sha256 asset (cached forever per tag). */
function release_sha256(array $r): ?string
{
    if (!empty($r['sha256'])) {
        return $r['sha256'];
    }
    if (empty($r['sha256_url']) || $r['tag'] === '') {
        return null;
    }
    $key = 'sha-' . preg_replace('/[^A-Za-z0-9._-]/', '_', $r['tag']) . '.json';
    $cached = cache_read($key, PHP_INT_MAX);
    if (is_string($cached['data'])) {
        return $cached['data'];
    }
    $body = http_get($r['sha256_url'], false);
    if ($body !== null && preg_match('/\b([a-f0-9]{64})\b/i', $body, $m)) {
        $sha = strtolower($m[1]);
        cache_write($key, $sha);
        return $sha;
    }
    return null;
}
