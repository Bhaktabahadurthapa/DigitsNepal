import { beforeEach, expect, it } from "vitest";
import { addHistory, loadHistory } from "../src/storage";

beforeEach(() => localStorage.clear());

it("keeps only the latest version of a sentence typed live", () => {
  const base = { mode: "accurate" as const, english: "", nepali: null };
  addHistory({ ...base, id: "1", at: 1000, text: "Malai" });
  addHistory({ ...base, id: "2", at: 2000, text: "Malai thaha" });
  addHistory({ ...base, id: "3", at: 3000, text: "Malai thaha chaina" });
  expect(loadHistory().map((h) => h.text)).toEqual(["Malai thaha chaina"]);
  addHistory({ ...base, id: "4", at: 4000, text: "Bholi aaunchu" });
  expect(loadHistory().map((h) => h.text)).toEqual(["Bholi aaunchu", "Malai thaha chaina"]);
});
