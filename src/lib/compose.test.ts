import { describe, expect, it } from "vitest";
import { endsSentence, midWord, PAUSE_MS, shouldTranslate, splitSentences } from "./compose";

describe("live compose timing", () => {
  it("does not translate empty or unchanged text", () => {
    expect(shouldTranslate("   ", "", 10_000)).toBe(false);
    expect(shouldTranslate("Yo ho.", "Yo ho.", 10_000)).toBe(false);
  });

  it("translates immediately at a sentence boundary", () => {
    expect(shouldTranslate("Yo error kina aayo? ", "", 0)).toBe(true);
    expect(shouldTranslate("समाधान स्पष्ट भएन।", "", 0)).toBe(true);
  });

  it("waits while a word is half typed", () => {
    expect(midWord("Maile hij")).toBe(true);
    expect(shouldTranslate("Maile hij", "", PAUSE_MS)).toBe(false);
    expect(shouldTranslate("Maile hij", "", PAUSE_MS * 2)).toBe(true);
  });

  it("translates after a pause when the word is complete", () => {
    expect(shouldTranslate("Maile hijo ", "", PAUSE_MS - 1)).toBe(false);
    expect(shouldTranslate("Maile hijo ", "", PAUSE_MS)).toBe(true);
  });

  it("detects sentence ends", () => {
    expect(endsSentence("Done.")).toBe(true);
    expect(endsSentence("v1.2")).toBe(false);
    expect(endsSentence("Hello")).toBe(false);
  });

  it("splits sentences including danda", () => {
    expect(splitSentences("Ma aaye। Timi kaha? OK.")).toEqual(["Ma aaye।", "Timi kaha?", "OK."]);
  });
});
