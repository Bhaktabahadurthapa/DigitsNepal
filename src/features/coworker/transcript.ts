// Pure reducer from streamed coworker events to what the chat shows.
import type { CoworkerEvent, Decision } from "./api";

export type Item =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string; streaming: boolean }
  | { kind: "tool"; tool: string; callId: string; done: boolean }
  | { kind: "approval"; id: string; tool: string; args: unknown; reason: string; standingOk: boolean; hardFloor: boolean; decision?: Decision }
  | { kind: "denied"; tool: string; reason: string }
  | { kind: "question"; id: string; question: string; options: string[]; answered?: string }
  | { kind: "error"; text: string };

export interface Transcript { items: Item[]; running: boolean }
export const empty: Transcript = { items: [], running: false };

function lastAssistant(items: Item[]): number {
  const i = items.length - 1;
  return i >= 0 && items[i]!.kind === "assistant" && (items[i] as { streaming: boolean }).streaming ? i : -1;
}

export function reduce(t: Transcript, e: CoworkerEvent): Transcript {
  const items = t.items.slice();
  switch (e.type) {
    case "status":
      return { items: e.state === "idle" ? items.map((x) => (x.kind === "assistant" ? { ...x, streaming: false } : x)) : items, running: e.state === "running" };
    case "text": {
      const i = lastAssistant(items);
      if (i >= 0) items[i] = { ...(items[i] as Extract<Item, { kind: "assistant" }>), text: (items[i] as { text: string }).text + e.text };
      else items.push({ kind: "assistant", text: e.text, streaming: true });
      return { ...t, items };
    }
    case "reset": {
      // A retried model attempt replaces the failed partial answer.
      const i = lastAssistant(items);
      if (i >= 0) items.splice(i, 1);
      return { ...t, items };
    }
    case "tool_started":
      closeStreaming(items);
      items.push({ kind: "tool", tool: e.tool, callId: e.call_id, done: false });
      return { ...t, items };
    case "tool_finished":
      return { ...t, items: items.map((x) => (x.kind === "tool" && x.callId === e.call_id ? { ...x, done: true } : x)) };
    case "approval_required":
      closeStreaming(items);
      items.push({ kind: "approval", id: e.id, tool: e.tool, args: e.arguments, reason: e.reason, standingOk: e.standing_ok, hardFloor: e.hard_floor });
      return { ...t, items };
    case "approval_resolved":
      return { ...t, items: items.map((x) => (x.kind === "approval" && x.id === e.id ? { ...x, decision: e.decision } : x)) };
    case "denied":
      items.push({ kind: "denied", tool: e.tool, reason: e.reason });
      return { ...t, items };
    case "question":
      items.push({ kind: "question", id: e.id, question: e.question, options: e.options });
      return { ...t, items };
    case "done": {
      const i = lastAssistant(items);
      if (i >= 0) items[i] = { kind: "assistant", text: e.text || (items[i] as { text: string }).text, streaming: false };
      else if (e.text) items.push({ kind: "assistant", text: e.text, streaming: false });
      return { ...t, items };
    }
    case "cancelled":
      items.push({ kind: "error", text: "Stopped." });
      return { ...t, items };
    case "error":
      items.push({ kind: "error", text: e.message });
      return { ...t, items };
    case "provider_error":
      return t;
  }
}

function closeStreaming(items: Item[]) {
  const i = lastAssistant(items);
  if (i >= 0) items[i] = { ...(items[i] as Extract<Item, { kind: "assistant" }>), streaming: false };
}
