// DigitsNepal site — interlinear demo + scroll reveals. No dependencies, no tracking.
(() => {
  "use strict";
  document.documentElement.classList.remove("no-js");
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Scroll reveals
  const io = new IntersectionObserver(
    (entries) => entries.forEach((e) => {
      if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
    }),
    { threshold: 0.12, rootMargin: "0px 0px -40px 0px" },
  );
  document.querySelectorAll(".reveal").forEach((el) => io.observe(el));

  // Copy buttons (e.g. SHA-256 on the download page)
  document.querySelectorAll("[data-copy]").forEach((b) =>
    b.addEventListener("click", async () => {
      const el = document.querySelector(b.dataset.copy);
      try { await navigator.clipboard.writeText(el.textContent.trim()); b.textContent = "Copied"; }
      catch { b.textContent = "Select & copy"; }
      setTimeout(() => (b.textContent = "Copy"), 1600);
    }),
  );

  // Illustrative examples (hand-written). [romanized, devanagari, gloss, isEnglishTerm]
  const EXAMPLES = [
    {
      topic: "at work",
      words: [
        ["Maile", "मैले", "I (doer)"],
        ["hijo", "हिजो", "yesterday"],
        ["meeting ma", "मिटिङमा", "in the meeting", 1],
        ["technical problem", "प्राविधिक समस्या", "technical problem", 1],
        ["ko barema", "को बारेमा", "about"],
        ["kura gareko thiye", "कुरा गरेको थिएँ", "had discussed"],
        ["tara", "तर", "but"],
        ["solution", "समाधान", "solution", 1],
        ["clear bhayena.", "स्पष्ट भएन।", "was not clear"],
      ],
      en: {
        accurate: "I discussed the technical problem during yesterday's meeting, but the solution was not clear.",
        professional: "During yesterday's meeting, I discussed the technical issue, but the solution remained unclear.",
        casual: "I brought up the tech problem in yesterday's meeting, but the fix still wasn't clear.",
      },
    },
    {
      topic: "on call",
      words: [
        ["Deployment", "डिप्लोयमेन्ट", "deployment", 1],
        ["complete bhayo", "पूरा भयो", "completed"],
        ["tara", "तर", "but"],
        ["pods", "पडहरू", "pods", 1],
        ["ready chainan.", "तयार छैनन्।", "are not ready"],
      ],
      en: {
        accurate: "The deployment completed, but the pods are not ready.",
        professional: "The deployment has completed, but the pods are not ready yet.",
        casual: "Deploy's done, but the pods aren't ready yet.",
      },
    },
    {
      topic: "in the chat",
      words: [
        ["Bholi ko", "भोलिको", "tomorrow's"],
        ["meeting ma", "मिटिङमा", "to the meeting", 1],
        ["ma", "म", "I"],
        ["ali", "अलि", "a little"],
        ["dhilo", "ढिलो", "late"],
        ["aauchu.", "आउँछु।", "will come"],
      ],
      en: {
        accurate: "I will come to tomorrow's meeting a little late.",
        professional: "I will be joining tomorrow's meeting a little late.",
        casual: "I'll be a bit late to tomorrow's meeting.",
      },
    },
  ];
  const NUM = ["१", "२", "३"];

  const lin = document.getElementById("interlinear");
  const enEl = document.getElementById("english");
  const srEl = document.getElementById("sr-english");
  const exTabs = [...document.querySelectorAll("[data-ex]")];
  const modeTabs = [...document.querySelectorAll("[data-mode]")];
  if (!lin) return; // only the home page has the folio

  let ex = 0;
  let mode = "accurate";
  let run = 0; // cancels in-flight animations
  let autoplay = !reduce;
  let autoTimer;

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  function select(tabs, attr, value) {
    tabs.forEach((t) => t.setAttribute("aria-selected", String(t.dataset[attr] === String(value))));
  }

  async function typeInto(el, text, speed, token) {
    if (reduce) { el.textContent = text; return; }
    el.textContent = "";
    for (const ch of text) {
      if (token !== run) return;
      el.textContent += ch;
      await sleep(speed + Math.random() * speed * 0.8);
    }
  }

  async function typeEnglish(token) {
    const text = EXAMPLES[ex].en[mode];
    srEl.textContent = text;
    await typeInto(enEl, text, 16, token);
  }

  async function play(i) {
    const token = ++run;
    ex = i;
    clearTimeout(autoTimer);
    select(exTabs, "ex", i);
    document.getElementById("folio-num").textContent = NUM[i];
    document.getElementById("folio-topic").textContent = EXAMPLES[i].topic;
    lin.textContent = "";
    enEl.textContent = "";

    for (const [ro, dv, gs, isEn] of EXAMPLES[i].words) {
      if (token !== run) return;
      const col = document.createElement("span");
      col.className = "gl" + (isEn ? " en" : "");
      col.innerHTML = '<span class="ro"></span><span class="dv" lang="ne"></span><span class="gs"></span>';
      col.querySelector(".dv").textContent = dv;
      col.querySelector(".gs").textContent = gs;
      lin.appendChild(col);
      await typeInto(col.querySelector(".ro"), ro, 34, token);
      if (token !== run) return;
      col.classList.add("shown");
      if (!reduce) await sleep(120);
    }
    if (!reduce) await sleep(260);
    await typeEnglish(token);
    if (token === run && autoplay) {
      autoTimer = setTimeout(() => play((ex + 1) % EXAMPLES.length), 5200);
    }
  }

  exTabs.forEach((t) =>
    t.addEventListener("click", () => {
      autoplay = false; // the reader took over
      play(Number(t.dataset.ex));
    }),
  );
  modeTabs.forEach((t) =>
    t.addEventListener("click", () => {
      autoplay = false;
      clearTimeout(autoTimer);
      mode = t.dataset.mode;
      select(modeTabs, "mode", mode);
      const token = ++run;
      // keep the interlinear, rewrite only the English line
      lin.querySelectorAll(".gl").forEach((g) => g.classList.add("shown"));
      typeEnglish(token);
    }),
  );

  // Start when the folio is on screen; pause when it scrolls away.
  const folio = document.querySelector(".folio");
  let started = false;
  new IntersectionObserver(
    (entries) => {
      entries.forEach((e) => {
        if (e.isIntersecting && !started) {
          started = true;
          setTimeout(() => play(0), reduce ? 0 : 700);
        }
      });
    },
    { threshold: 0.3 },
  ).observe(folio);

})();
