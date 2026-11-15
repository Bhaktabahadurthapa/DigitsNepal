<?php
/**
 * digitsnepal.com — front controller.
 * Every URL is routed here by .htaccess; pages are whitelisted below, so
 * no user input ever reaches an include path.
 */
declare(strict_types=1);

define('BASE', rtrim(str_replace('\\', '/', dirname($_SERVER['SCRIPT_NAME'] ?? '/index.php')), '/'));

require __DIR__ . '/inc/layout.php';
require __DIR__ . '/inc/release.php';
require __DIR__ . '/inc/markdown.php';

$PAGES = [
    'home' => [
        'title' => 'DigitsNepal — Every Nepali word has meaning',
        'desc'  => 'DigitsNepal turns Romanized Nepali, Devanagari and mixed Nepali-English into natural English as you type — on macOS, in any app, powered by DigitsNepalAI.',
        'nav'   => 'home',
    ],
    'dictionary' => [
        'title' => 'Dictionary — DigitsNepal',
        'desc'  => 'Search Romanized Nepali words and phrases, see their Devanagari spelling and meaning, and gloss a whole sentence word by word.',
        'nav'   => 'dictionary',
    ],
    'download' => [
        'title' => 'Download — DigitsNepal',
        'desc'  => 'Download DigitsNepal for Mac — free and open source. Latest version, checksum and install guide.',
        'nav'   => 'download',
    ],
    'changelog' => [
        'title' => 'Changelog — DigitsNepal',
        'desc'  => 'Every DigitsNepal release, newest first.',
        'nav'   => 'changelog',
    ],
    'privacy' => [
        'title' => 'Privacy — DigitsNepal',
        'desc'  => 'What DigitsNepal and DigitsNepalAI do — and never do — with your words.',
        'nav'   => 'privacy',
    ],
];

$slug = isset($_GET['page']) && is_string($_GET['page']) ? $_GET['page'] : 'home';
if ($slug === '') {
    $slug = 'home';
}
if (isset($PAGES[$slug])) {
    $page = $PAGES[$slug];
} else {
    http_response_code(404);
    $slug = 'notfound';
    $page = ['title' => 'Page not found — DigitsNepal', 'desc' => 'This page does not exist.', 'nav' => ''];
}
$page['slug'] = $slug;

$release = latest_release();

header('Content-Type: text/html; charset=utf-8');
render_head($page);
render_header($page['nav'], $release);
echo '<main id="main">';
require __DIR__ . '/inc/pages/' . $slug . '.php';
echo '</main>';
render_footer();
