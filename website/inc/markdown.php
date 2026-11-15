<?php
/**
 * Minimal, safe Markdown for GitHub release notes.
 * Everything is HTML-escaped first; only a small set of constructs is then
 * turned back into tags. Links are limited to http(s).
 */
declare(strict_types=1);

function md_inline(string $escaped): string
{
    // `code`
    $s = preg_replace('/`([^`]+)`/', '<code>$1</code>', $escaped);
    // **bold**
    $s = preg_replace('/\*\*([^*]+)\*\*/', '<strong>$1</strong>', $s);
    // [text](https://url) — URL already escaped, so quotes cannot break out.
    $s = preg_replace_callback('/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/', static function (array $m): string {
        return '<a href="' . $m[2] . '" rel="noopener">' . $m[1] . '</a>';
    }, $s);
    // *em* / _em_
    $s = preg_replace('/(?<![*\w])\*([^*\n]+)\*(?![*\w])/', '<em>$1</em>', $s);
    return (string) $s;
}

function markdown(string $md): string
{
    $lines = preg_split('/\r\n|\r|\n/', $md) ?: [];
    $html = '';
    $list = null;   // 'ul' | 'ol' | null
    $para = [];

    $flushPara = static function () use (&$para, &$html): void {
        if ($para) {
            $html .= '<p>' . md_inline(implode(' ', $para)) . "</p>\n";
            $para = [];
        }
    };
    $closeList = static function () use (&$list, &$html): void {
        if ($list !== null) {
            $html .= "</$list>\n";
            $list = null;
        }
    };

    foreach ($lines as $raw) {
        $line = e(rtrim($raw));
        if (trim($line) === '') {
            $flushPara();
            $closeList();
            continue;
        }
        if (preg_match('/^(#{1,4})\s+(.*)$/', $line, $m)) {
            $flushPara();
            $closeList();
            $level = min(4, strlen($m[1]) + 2); // ### → h4 at most
            $html .= "<h$level>" . md_inline($m[2]) . "</h$level>\n";
            continue;
        }
        if (preg_match('/^\s*[-*]\s+(.*)$/', $line, $m) || preg_match('/^\s*\d+[.)]\s+(.*)$/', $line, $m2)) {
            $flushPara();
            $type = isset($m[1]) ? 'ul' : 'ol';
            $item = $m[1] ?? $m2[1];
            if ($list !== $type) {
                $closeList();
                $html .= "<$type>\n";
                $list = $type;
            }
            $html .= '<li>' . md_inline($item) . "</li>\n";
            continue;
        }
        $closeList();
        $para[] = trim($line);
    }
    $flushPara();
    $closeList();
    return $html;
}
