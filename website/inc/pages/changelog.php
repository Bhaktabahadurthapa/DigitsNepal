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
    // Older release notes end with a raw "SHA-256: `…`" checksum line; it
    // means nothing to most visitors, so it is not shown.
    $notes = trim(preg_replace('/^[ \t]*SHA-?256:?[ \t]*`?[0-9a-fA-F]{64}`?[ \t]*\R?/mi', '', $r['notes']));
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
      <p class="rel-links">
        <?php if (!empty($r['dmg'])): ?><a href="<?= e($r['dmg']['url']) ?>">Download <?= e($r['dmg']['name']) ?></a> · <?php endif; ?>
        <a href="<?= e($r['url']) ?>">View on GitHub</a>
      </p>
    </div>
  </article>
<?php endforeach; ?>
</section>
