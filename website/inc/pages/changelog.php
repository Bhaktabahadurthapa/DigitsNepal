<?php
$releases = all_releases();
?>
<section class="page-head">
  <div class="wrap">
    <p class="kicker rise" style="--d:0">Changelog</p>
    <h1 class="rise" style="--d:1">Every release, newest first.</h1>
    <p class="lede rise" style="--d:2">Pulled live from GitHub — this page updates itself whenever a new version ships.</p>
  </div>
</section>

<section class="wrap changelog">
<?php foreach ($releases as $i => $r):
    $ver = ltrim($r['tag'], 'v');
    // Release notes on GitHub end with a raw "SHA-256: `…`" line. Pull it out
    // and show it as a tidy, collapsible "verify your download" box instead.
    $notes = $r['notes'];
    $sha = null;
    if (preg_match('/^[ \t]*SHA-?256:?[ \t]*`?([0-9a-fA-F]{64})`?[ \t]*$/m', $notes, $m)) {
        $sha = strtolower($m[1]);
        $notes = trim(str_replace($m[0], '', $notes));
    }
    if ($sha === null && !empty($r['dmg'])) {
        $sha = release_sha256($r);
    }
    $shaId = 'sha-' . preg_replace('/[^A-Za-z0-9_-]/', '-', $r['tag']);
?>
  <article class="rel reveal" id="<?= e($r['tag']) ?>">
    <div class="rel-side">
      <span class="rel-num"><?= e(deva_digits($ver)) ?></span>
      <time datetime="<?= e($r['date']) ?>"><?= e(nice_date($r['date'])) ?></time>
      <?php if ($i === 0): ?><span class="tag-latest">Latest</span><?php endif; ?>
      <?php if (!empty($r['prerelease'])): ?><span class="tag-pre">Pre-release</span><?php endif; ?>
    </div>
    <div class="rel-body prose">
      <h2><a href="#<?= e($r['tag']) ?>"><?= e($r['name']) ?></a></h2>
      <?= $notes !== '' ? markdown($notes) : '<p>No release notes.</p>' ?>
      <?php if ($sha && !empty($r['dmg'])): ?>
      <details class="rel-verify">
        <summary>Verify your download <span>(optional)</span></summary>
        <p>The SHA-256 checksum is the file's fingerprint. If yours matches, the download is complete and unchanged.</p>
        <div class="sha"><code id="<?= e($shaId) ?>"><?= e($sha) ?></code> <button type="button" class="copy" data-copy="#<?= e($shaId) ?>">Copy</button></div>
        <p class="verify">In Terminal: <code>shasum -a 256 ~/Downloads/<?= e($r['dmg']['name']) ?></code></p>
      </details>
      <?php endif; ?>
      <p class="rel-links">
        <?php if (!empty($r['dmg'])): ?><a href="<?= e($r['dmg']['url']) ?>">Download <?= e($r['dmg']['name']) ?></a> · <?php endif; ?>
        <a href="<?= e($r['url']) ?>">View on GitHub</a>
      </p>
    </div>
  </article>
<?php endforeach; ?>
</section>
