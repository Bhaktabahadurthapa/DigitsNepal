// Structural checks for the evaluation dataset (run in CI).
import { readFileSync } from "node:fs";

const data = JSON.parse(readFileSync(new URL("../tests/fixtures/eval-dataset.json", import.meta.url), "utf8"));
const langs = new Set(["romanized_nepali", "native_nepali", "english", "mixed"]);
const modes = new Set(["accurate", "professional", "casual", "academic", "technical", "grammar"]);
const errors = [];
const ids = new Set();
for (const c of data.cases) {
  if (ids.has(c.id)) errors.push(`duplicate id ${c.id}`);
  ids.add(c.id);
  if (!c.source?.trim()) errors.push(`${c.id}: empty source`);
  if (!c.reference?.trim()) errors.push(`${c.id}: empty reference`);
  if (!langs.has(c.expected_language)) errors.push(`${c.id}: bad language ${c.expected_language}`);
  if (!modes.has(c.mode)) errors.push(`${c.id}: bad mode ${c.mode}`);
  for (const k of c.must_keep) if (!c.source.includes(k)) errors.push(`${c.id}: must_keep '${k}' not in source`);
}
const cats = {};
for (const c of data.cases) cats[c.category] = (cats[c.category] ?? 0) + 1;
if (data.cases.length < 100) errors.push(`only ${data.cases.length} cases (need ≥100)`);
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(`eval dataset OK: ${data.cases.length} cases`, cats);
