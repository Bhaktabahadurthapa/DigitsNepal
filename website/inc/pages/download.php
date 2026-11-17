<?php
/** @var array $release */
$dmg = $release['dmg'];
$ver = ltrim($release['tag'], 'v');
?>
<section class="page-head">
  <div class="wrap">
    <p class="kicker rise" style="--d:0">Download · अंक <?= e(deva_digits($ver)) ?></p>
    <h1 class="rise" style="--d:1">DigitsNepal for Mac.</h1>
    <p class="lede rise" style="--d:2">Free and open source. Always the latest release, straight from GitHub.</p>
  </div>
</section>

<section class="wrap release-layout">
  <article class="release-card rise" style="--d:3">
    <header>
      <svg class="release-mark" aria-hidden="true"><use href="#pennant"/></svg>
      <div>
        <h2><?= e($release['name']) ?></h2>
        <p class="meta">Released <?= e(nice_date($release['date'])) ?><?php if ($release['downloads'] > 0): ?> · <?= e(number_format($release['downloads'])) ?> downloads<?php endif; ?></p>
      </div>
    </header>
    <a class="btn btn-ink btn-lg" href="<?= e($dmg['url']) ?>">Download .dmg <span class="btn-meta"><?= e(human_size($dmg['size'])) ?></span></a>
    <dl class="facts">
      <div><dt>File</dt><dd><?= e($dmg['name']) ?></dd></div>
      <div><dt>For</dt><dd>Apple Silicon Macs · macOS 12 Monterey or later</dd></div>
    </dl>
  </article>

  <aside class="side-notes rise" style="--d:4">
    <h3>Intel Mac?</h3>
    <p>Build from source in a few commands — see the <a href="https://github.com/Bhaktabahadurthapa/DigitsNepal#install">README</a>.</p>
    <h3>Older versions</h3>
    <p><a href="<?= e(url('changelog')) ?>">Changelog</a> · <a href="https://github.com/Bhaktabahadurthapa/DigitsNepal/releases">all releases on GitHub</a></p>
  </aside>
</section>

<section class="wrap guide">
  <header class="sec-head reveal">
    <p class="sec-no">§ <span class="deva-num">१</span></p>
    <h2>Install in four steps.</h2>
  </header>
  <ol class="install install-wide reveal">
    <li><span>१</span><p>Open the .dmg and drag <b>DigitsNepal</b> into <b>Applications</b>.</p></li>
    <li><span>२</span><p>First launch: <b>right-click DigitsNepal → Open</b>, then <b>Open</b>. This build isn't notarized by Apple yet, so macOS asks once.</p></li>
    <li><span>३</span><p>In <b>Settings</b>, choose the model that powers <b>DigitsNepalAI</b> — a free local model with Ollama or LM Studio, or your own key for a cloud provider.</p></li>
    <li><span>४</span><p>Optional: for <kbd>⌃⌥T</kbd> in other apps, allow DigitsNepal in <b>System Settings › Privacy &amp; Security › Accessibility</b>.</p></li>
  </ol>

  <header class="sec-head reveal" style="margin-top:80px">
    <p class="sec-no">§ <span class="deva-num">२</span></p>
    <h2>If something's off.</h2>
  </header>
  <div class="faq reveal">
    <details>
      <summary>“DigitsNepal can't be opened because Apple cannot check it.”</summary>
      <p>Right-click the app in Applications and choose <b>Open</b>. You only need to do this once per version.</p>
    </details>
    <details>
      <summary>macOS asks for my Keychain password.</summary>
      <p>DigitsNepal stores API keys in your Keychain. Choose <b>Always Allow</b>; after an update macOS may ask once more because unsigned builds change identity.</p>
    </details>
    <details>
      <summary>The ⌃⌥T shortcut does nothing.</summary>
      <p>Allow DigitsNepal under <b>Privacy &amp; Security › Accessibility</b>. After an update, remove the old entry and add the app again.</p>
    </details>
    <details>
      <summary>No English appears.</summary>
      <p>Open <b>Settings</b>: the checklist shows what DigitsNepalAI still needs — a model, an API key, or permission to use a cloud provider. <b>Test connection</b> shows the exact error.</p>
    </details>
  </div>
</section>
