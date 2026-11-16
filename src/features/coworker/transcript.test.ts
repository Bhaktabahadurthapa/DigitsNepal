import { describe, expect, it } from "vitest";
import { empty, reduce } from "./transcript";
import type { CoworkerEvent } from "./api";

const s = "s1";
const run = (events: Array<Record<string, unknown>>) => events.reduce((t, e) => reduce(t, { session: s, ...e } as CoworkerEvent), empty);

describe("coworker transcript", () => {
  it("streams text, shows tool activity and the final answer", () => {
    const t = run([
      { type: "status", state: "running" },
      { type: "text", text: "Understood: " },
      { type: "tool_started", tool: "dn.search_files", call_id: "c1", arguments: {} },
      { type: "tool_finished", tool: "dn.search_files", call_id: "c1" },
      { type: "text", text: "Found 2 PDFs." },
      { type: "done", text: "Found 2 PDFs." },
      { type: "status", state: "idle" },
    ]);
    expect(t.running).toBe(false);
    expect(t.items.map((i) => i.kind)).toEqual(["assistant", "tool", "assistant"]);
    expect(t.items[1]).toMatchObject({ done: true });
    expect(t.items[2]).toMatchObject({ text: "Found 2 PDFs.", streaming: false });
  });

  it("records approval decisions on the card", () => {
    const t = run([
      { type: "approval_required", id: "a1", tool: "dn.write_file", arguments: { path: "/x" }, reason: "r", standing_ok: true, hard_floor: false },
      { type: "approval_resolved", id: "a1", decision: "deny" },
    ]);
    expect(t.items[0]).toMatchObject({ kind: "approval", decision: "deny", standingOk: true });
  });

  it("drops a failed attempt's partial text on retry", () => {
    const t = run([{ type: "text", text: "garbled" }, { type: "reset" }, { type: "text", text: "clean" }]);
    expect(t.items).toEqual([{ kind: "assistant", text: "clean", streaming: true }]);
  });

  it("shows denials and errors", () => {
    const t = run([{ type: "denied", tool: "dn.read_file", reason: "outside" }, { type: "error", message: "model failed" }]);
    expect(t.items.map((i) => i.kind)).toEqual(["denied", "error"]);
  });
});
