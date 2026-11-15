// Dictionary page: instant search, category filter, and an offline sentence
// glosser that mirrors the app's longest-match lookup. Runs fully in the browser.
(() => {
  "use strict";
  const grid = document.getElementById("lex-grid");
  if (!grid) return;
  const base = document.querySelector('link[rel="icon"]').getAttribute("href").replace(/favicon\.png$/, "");
  const items = [...grid.children];
  const q = document.getElementById("lex-q");
  const count = document.getElementById("lex-count");
  const empty = document.getElementById("lex-empty");
  const chips = [...document.querySelectorAll(".chip")];
  let cat = "";

  const norm = (s) => s.toLowerCase().normalize("NFC").replace(/\s+/g, " ").trim();

  function filter() {
    const needle = norm(q.value);
    let shown = 0;
    for (const li of items) {
      const ok = (!cat || li.dataset.cat === cat) && (!needle || li.dataset.s.includes(needle));
      li.hidden = !ok;
      if (ok) shown++;
    }
    count.textContent = `${shown} ${shown === 1 ? "entry" : "entries"}`;
    empty.hidden = shown !== 0;
  }
  q.addEventListener("input", filter);
  chips.forEach((c) =>
    c.addEventListener("click", () => {
      cat = c.dataset.cat;
      chips.forEach((x) => x.setAttribute("aria-pressed", String(x === c)));
      filter();
    }),
  );
  // Deep link: /dictionary?q=chaina
  const initial = new URLSearchParams(location.search).get("q");
  if (initial) { q.value = initial; filter(); }

  // ---- Sentence glosser ----
  const input = document.getElementById("gloss-input");
  const out = document.getElementById("gloss-out");
  let phrases = [];
  const words = new Map();

  fetch(base + "data/lexicon.json")
    .then((r) => r.json())
    .then((lex) => {
      phrases = lex.phrases.flatMap((p) => p.roman.map((r) => ({ toks: r.split(" "), e: p })));
      phrases.sort((a, b) => b.toks.length - a.toks.length);
      for (const w of lex.words) for (const r of w.roman) if (!words.has(r)) words.set(r, w);
      render();
    })
    .catch(() => { out.textContent = "Couldn't load the dictionary."; });

  function tokenize(text) {
    const toks = [];
    const re = /[A-Za-z']+|[ऀ-ॿ]+|[^\sA-Za-z'ऀ-ॿ]+/g;
    let m;
    while ((m = re.exec(text))) toks.push(m[0]);
    return toks;
  }

  function gloss(text) {
    const toks = tokenize(text);
    const res = [];
    let i = 0;
    outer: while (i < toks.length) {
      const t = toks[i];
      if (!/[A-Za-z]/.test(t)) {
        if (/^[.,!?।;:]+$/.test(t) && res.length) res[res.length - 1].ro += t;
        else if (!/^\s*$/.test(t)) res.push({ ro: t, dv: "", gs: "" });
        i++;
        continue;
      }
      for (const p of phrases) {
        const n = p.toks.length;
        const slice = toks.slice(i, i + n).map((x) => x.toLowerCase());
        if (slice.length === n && slice.join(" ") === p.toks.join(" ")) {
          res.push({ ro: toks.slice(i, i + n).join(" "), dv: p.e.deva, gs: p.e.gloss, amb: p.e.ambiguous });
          i += n;
          continue outer;
        }
      }
      const w = words.get(t.toLowerCase());
      res.push(w ? { ro: t, dv: w.deva, gs: w.gloss, amb: w.ambiguous } : { ro: t, dv: "", gs: "", en: true });
      i++;
    }
    return res;
  }

  function render() {
    out.textContent = "";
    for (const g of gloss(input.value)) {
      const col = document.createElement("span");
      col.className = "gl shown" + (g.en ? " en" : "") + (g.amb ? " amb" : "");
      const ro = document.createElement("span"); ro.className = "ro"; ro.textContent = g.ro;
      const dv = document.createElement("span"); dv.className = "dv"; dv.lang = "ne"; dv.textContent = g.dv || " ";
      const gs = document.createElement("span"); gs.className = "gs";
      gs.textContent = g.en ? "not in dictionary" : g.gs.split(";")[0] + (g.amb ? " ?" : "");
      if (g.en) gs.classList.add("miss");
      col.append(ro, dv, gs);
      out.appendChild(col);
    }
  }
  let t;
  input.addEventListener("input", () => { clearTimeout(t); t = setTimeout(render, 120); });
})();
