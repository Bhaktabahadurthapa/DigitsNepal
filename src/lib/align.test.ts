import { describe, expect, it } from "vitest";
import { alignMeanings } from "./align";
import type { WordMeaning } from "../types/translation";

const wm = (source: string): WordMeaning => ({ source, meaning: "m", context: "", uncertain: false, origin: "model" });

describe("alignMeanings", () => {
  const text = "Maile hijo meeting ma technical problem ko barema kura gareko thiye.";

  it("aligns words and phrases in order and preserves original text", () => {
    const segs = alignMeanings(text, [wm("Maile"), wm("hijo"), wm("ko barema"), wm("kura gareko thiye")]);
    expect(segs.map((s) => s.text).join("")).toBe(text);
    expect(segs.filter((s) => s.index !== null).map((s) => s.text)).toEqual(["Maile", "hijo", "ko barema", "kura gareko thiye"]);
  });

  it("matches whole words only", () => {
    const segs = alignMeanings("Maile ma", [wm("ma")]);
    const hit = segs.find((s) => s.index === 0)!;
    expect(hit.text).toBe("ma");
    expect(segs[0].text).toBe("Maile ");
  });

  it("ignores entries not in the text and overlaps", () => {
    const segs = alignMeanings("tara solution", [wm("invented"), wm("tara"), wm("tara solution")]);
    expect(segs.filter((s) => s.index !== null)).toHaveLength(1);
  });

  it("works with Devanagari", () => {
    const segs = alignMeanings("मैले हिजो", [wm("हिजो")]);
    expect(segs.find((s) => s.index === 0)?.text).toBe("हिजो");
  });
});
