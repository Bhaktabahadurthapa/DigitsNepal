<?php
/** @var array $release */
$dmg = $release['dmg'];
$ver = ltrim($release['tag'], 'v');
?>
<section class="hero" id="top">
  <div class="arth" aria-hidden="true">अर्थ</div>
  <div class="wrap hero-grid">
    <div class="hero-copy">
      <p class="kicker rise" style="--d:0">Nepali → English · for macOS</p>
      <h1 class="rise" style="--d:1">Every Nepali word has <em>meaning</em>.</h1>
      <p class="deva-line rise" style="--d:2" lang="ne">हरेक नेपाली शब्दको अर्थ हुन्छ।</p>
      <p class="lede rise" style="--d:3">Write the way you think — Romanized Nepali, देवनागरी, or a mix with English. DigitsNepalAI sets it beside natural English while you type, ready to copy into Slack, email, or the meeting chat.</p>
      <div class="actions rise" style="--d:4">
        <a class="btn btn-ink" href="<?= e(url('download')) ?>">Download for Mac <span class="btn-meta"><?= e(human_size($dmg['size'])) ?></span></a>
        <a class="btn btn-line" href="<?= e(url('dictionary')) ?>">Try the dictionary</a>
      </div>
      <p class="fineprint rise" style="--d:5">v<?= e($ver) ?> · macOS 12+ · Apple Silicon · MIT licence · no account · no tracking</p>
    </div>

    <figure class="folio rise" style="--d:3" aria-labelledby="folio-cap">
      <div class="folio-head">
        <span class="folio-no">Folio <b id="folio-num">१</b> — <span id="folio-topic">at work</span></span>
        <div class="folio-pick" role="tablist" aria-label="Example sentences">
          <button role="tab" data-ex="0" aria-selected="true">१</button>
          <button role="tab" data-ex="1" aria-selected="false">२</button>
          <button role="tab" data-ex="2" aria-selected="false">३</button>
        </div>
      </div>
      <div class="interlinear" id="interlinear" aria-hidden="true"></div>
      <div class="folio-rule"><span>English</span>
        <div class="modes" role="tablist" aria-label="Writing mode">
          <button role="tab" data-mode="accurate" aria-selected="true">Accurate</button>
          <button role="tab" data-mode="professional" aria-selected="false">Professional</button>
          <button role="tab" data-mode="casual" aria-selected="false">Casual</button>
        </div>
      </div>
      <p class="english" aria-hidden="true"><span id="english"></span><span class="caret"></span></p>
      <p class="sr-only" aria-live="polite" id="sr-english"></p>
      <figcaption id="folio-cap">Interlinear view — each word, its script, its meaning. <i>Example DigitsNepalAI output; results depend on the model you choose.</i></figcaption>
    </figure>
  </div>
</section>

<section class="entries" id="entries">
  <div class="wrap">
    <header class="sec-head reveal">
      <p class="sec-no">§ <span class="deva-num">१</span></p>
      <h2>A small dictionary of what it does.</h2>
    </header>
    <ol class="dict">
      <li class="entry reveal">
        <span class="num">१</span>
        <h3>live <span class="ne" lang="ne">प्रत्यक्ष</span></h3>
        <p class="pos">adj. — as you type</p>
        <p>English appears while you write: on a short pause, or the moment a sentence ends. Never mid-word. No translate button to hunt for.</p>
      </li>
      <li class="entry reveal">
        <span class="num">२</span>
        <h3>anywhere <span class="ne" lang="ne">जहाँसुकै</span></h3>
        <p class="pos">adv. — in any app</p>
        <p>Select text in Slack, Mail, Notes or your browser and press <kbd>⌃</kbd><kbd>⌥</kbd><kbd>T</kbd>. Read it, copy it, or replace your words in place — with undo.</p>
      </li>
      <li class="entry reveal">
        <span class="num">३</span>
        <h3>meaning <span class="ne" lang="ne">अर्थ</span></h3>
        <p class="pos">n. — word by word</p>
        <p>Click any word or phrase — <i>ko barema</i>, <i>kura gareko thiye</i> — for its meaning in this sentence, its literal sense, and its Devanagari spelling. <a href="<?= e(url('dictionary')) ?>">Browse the dictionary →</a></p>
      </li>
      <li class="entry reveal">
        <span class="num">४</span>
        <h3>register <span class="ne" lang="ne">शैली</span></h3>
        <p class="pos">n. — six ways to say it</p>
        <p>Accurate, Professional, Casual, Academic, Technical, Grammar. The faithful translation always stays beside the polished one — tense, negation and certainty intact.</p>
      </li>
      <li class="entry reveal">
        <span class="num">५</span>
        <h3>verbatim <span class="ne" lang="ne">ज्यूँका त्यूँ</span></h3>
        <p class="pos">adv. — for engineers</p>
        <p>Kubernetes, AWS, pods, PR numbers, error codes and versions pass through untouched. If a <i>chaina</i> loses its “not”, DigitsNepalAI warns you.</p>
      </li>
      <li class="entry reveal">
        <span class="num">६</span>
        <h3>yours <span class="ne" lang="ne">तपाईंको</span></h3>
        <p class="pos">pron. — no account</p>
        <p>No sign-up, no analytics. Run DigitsNepalAI on a free model on your Mac, or with a key from the provider you already use. Keys stay in the macOS Keychain.</p>
      </li>
    </ol>
  </div>
</section>

<section class="meetings" id="meetings">
  <div class="wrap meet-grid">
    <div class="meet-copy reveal">
      <p class="sec-no">§ <span class="deva-num">२</span></p>
      <h2>In the meeting, <em>only you</em> see it.</h2>
      <p>Overlay mode shrinks DigitsNepal to a see-through panel that floats above Zoom, Meet or Teams. Turn on <b>Hide in share</b> and macOS leaves it out of screen sharing and recordings — your notes, your Nepali, your English.</p>
      <ul class="ticks">
        <li>Always on top, on every Space</li>
        <li>Adjustable transparency — text stays crisp</li>
        <li>One tap: copy English into the chat</li>
      </ul>
      <p class="aside">Check your meeting app's share preview once; a few capture tools ignore the hide request.</p>
    </div>
    <div class="call reveal" aria-label="Illustration: a video call with the DigitsNepal overlay visible only to you" role="img">
      <div class="call-bar"><span></span><span></span><span></span><b>Weekly sync</b><i class="rec">● Sharing screen</i></div>
      <div class="tiles">
        <div class="tile" style="--h:18"><span>AK</span></div>
        <div class="tile" style="--h:200"><span>SR</span></div>
        <div class="tile" style="--h:28"><span>PM</span></div>
        <div class="tile" style="--h:150"><span>You</span></div>
      </div>
      <div class="overlay-panel">
        <div class="op-head"><span>DigitsNepalAI</span><span class="badge">◐ Hidden in share</span></div>
        <p class="op-src">Ma yo task bholi samma sakchu.</p>
        <p class="op-en">I can finish this task by tomorrow.</p>
        <span class="op-copy">Copy English</span>
      </div>
    </div>
  </div>
</section>

<section class="colophon" id="engine">
  <div class="wrap">
    <header class="sec-head reveal">
      <p class="sec-no">§ <span class="deva-num">३</span></p>
      <h2>DigitsNepalAI, on the model you choose.</h2>
      <p class="sec-lede">Local by default. Cloud only when you switch it on — provider by provider.</p>
    </header>
    <dl class="index reveal">
      <div><dt>Ollama</dt><dd class="local">on your Mac · free</dd></div>
      <div><dt>LM Studio</dt><dd class="local">on your Mac · free</dd></div>
      <div><dt>Anthropic Claude</dt><dd>your API key</dd></div>
      <div><dt>Google Gemini</dt><dd>your API key</dd></div>
      <div><dt>OpenAI</dt><dd>your API key</dd></div>
      <div><dt>OpenRouter</dt><dd>one key, many models</dd></div>
      <div><dt>Groq · Mistral · DeepSeek</dt><dd>your API key</dd></div>
      <div><dt>Together · xAI</dt><dd>your API key</dd></div>
      <div><dt>Any OpenAI-compatible server</dt><dd>vLLM, llama.cpp, gateways</dd></div>
    </dl>
  </div>
</section>

<section class="get" id="stamp">
  <div class="wrap get-grid">
    <div class="stamp reveal" aria-hidden="true">
      <div class="stamp-inner">
        <span class="stamp-top">नेपाल · DIGITS</span>
        <svg class="stamp-mark"><use href="#pennant"/></svg>
        <span class="stamp-word" lang="ne">अर्थ</span>
        <span class="stamp-val">रु <b>०</b></span>
        <span class="stamp-foot">v<?= e($ver) ?> · <?= e(deva_digits(date('Y'))) ?></span>
      </div>
    </div>
    <div class="get-copy reveal">
      <p class="sec-no">§ <span class="deva-num">४</span></p>
      <h2>Free. Yours to keep.</h2>
      <a class="btn btn-ink btn-lg" href="<?= e($dmg['url']) ?>">Download for Mac <span class="btn-meta">v<?= e($ver) ?> · <?= e(human_size($dmg['size'])) ?></span></a>
      <ol class="install">
        <li><span>१</span><p>Open the .dmg and drag <b>DigitsNepal</b> into <b>Applications</b>.</p></li>
        <li><span>२</span><p>First launch: <b>right-click → Open</b>. Early builds aren't notarized by Apple yet.</p></li>
        <li><span>३</span><p>In <b>Settings</b>, choose the model that powers <b>DigitsNepalAI</b>. Start typing.</p></li>
        <li><span>४</span><p>For <kbd>⌃⌥T</kbd> in other apps, allow DigitsNepal in <b>Privacy &amp; Security › Accessibility</b>.</p></li>
      </ol>
      <p class="aside"><a href="<?= e(url('download')) ?>">Checksum, requirements &amp; troubleshooting</a> · <a href="<?= e(url('changelog')) ?>">What's new</a></p>
    </div>
  </div>
</section>
