<?php
// Rendered server-side from the app's own lexicon (works without JavaScript);
// dictionary.js then adds instant search, filters and the sentence glosser.
$lex = json_decode((string) @file_get_contents(dirname(__DIR__, 2) . '/data/lexicon.json'), true) ?: ['phrases' => [], 'words' => []];
$entries = array_merge($lex['phrases'] ?? [], $lex['words'] ?? []);
$cats = [];
foreach ($entries as $en) {
    $cats[$en['cat']] = ($cats[$en['cat']] ?? 0) + 1;
}
$count = count($entries);
?>
<section class="page-head">
  <div class="wrap">
    <p class="kicker rise" style="--d:0">Dictionary · शब्दकोश</p>
    <h1 class="rise" style="--d:1">Look up a word. <em>Gloss</em> a sentence.</h1>
    <p class="lede rise" style="--d:2"><?= e(deva_digits((string) $count)) ?> everyday Romanized Nepali words and phrases — the same offline dictionary built into the app. Everything runs in your browser; nothing you type is sent anywhere.</p>
  </div>
</section>

<section class="wrap gloss-tool rise" style="--d:3" aria-labelledby="gloss-h">
  <h2 id="gloss-h" class="tool-title">Gloss a sentence</h2>
  <label class="sr-only" for="gloss-input">Sentence in Romanized Nepali</label>
  <textarea id="gloss-input" rows="2" spellcheck="false" placeholder="Type Romanized Nepali, e.g. Malai thaha chaina, bholi feri herchu.">Maile hijo meeting ma kura gareko thiye tara solution clear bhayena.</textarea>
  <div class="interlinear gloss-out" id="gloss-out" aria-live="polite"></div>
  <p class="tool-note">Dictionary glosses only — word by word, without sentence context. <b>DigitsNepalAI</b> in the app gives the full, natural English translation.</p>
</section>

<section class="wrap lexicon" aria-labelledby="lex-h">
  <div class="lex-bar">
    <h2 id="lex-h" class="tool-title">Browse</h2>
    <label class="sr-only" for="lex-q">Search the dictionary</label>
    <input id="lex-q" type="search" placeholder="Search: chaina, yesterday, मैले…" autocomplete="off">
    <span class="lex-count" id="lex-count" aria-live="polite"><?= e((string) $count) ?> entries</span>
  </div>
  <div class="chips" role="group" aria-label="Filter by category">
    <button type="button" class="chip" data-cat="" aria-pressed="true">All</button>
<?php foreach ($cats as $cat => $n): ?>
    <button type="button" class="chip" data-cat="<?= e($cat) ?>" aria-pressed="false"><?= e($cat) ?> <small><?= e((string) $n) ?></small></button>
<?php endforeach; ?>
  </div>

  <ul class="lex-grid" id="lex-grid">
<?php foreach ($entries as $en):
    $main = $en['roman'][0] ?? '';
    $alts = array_slice($en['roman'], 1);
    $search = strtolower(implode(' ', $en['roman']) . ' ' . $en['gloss']) . ' ' . $en['deva'];
?>
    <li class="lex" data-cat="<?= e($en['cat']) ?>" data-s="<?= e($search) ?>">
      <div class="lex-head">
        <b><?= e($main) ?></b>
        <span class="lex-deva" lang="ne"><?= e($en['deva']) ?></span>
      </div>
      <p class="lex-gloss"><?= e($en['gloss']) ?></p>
      <p class="lex-meta">
        <?php if ($alts): ?><span>also: <?= e(implode(', ', $alts)) ?></span><?php endif; ?>
        <?php if (!empty($en['ambiguous'])): ?><span class="amb">several meanings</span><?php endif; ?>
        <span class="cat"><?= e($en['cat']) ?></span>
      </p>
    </li>
<?php endforeach; ?>
  </ul>
  <p class="lex-empty" id="lex-empty" hidden>No match. Romanized spellings vary — try <i>chha</i> for <i>cha</i>, or search the English meaning.</p>
</section>
<script src="<?= e(url('dictionary.js')) ?>?v=1" defer></script>
