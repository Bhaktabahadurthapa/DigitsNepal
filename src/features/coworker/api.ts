// Typed wrappers for the Sahayogi and Activity commands.
import { invoke } from "@tauri-apps/api/core";

export type Decision = "once" | "session" | "deny";
export interface Grant { id: number; path: string; created_at: number }
export interface SessionRow { id: string; agent: string; specialist: string; title: string; created_at: number; updated_at: number }
export interface Specialist { key: string; name: string; nepali: string; summary: string; icon: string; version: number; tools: string[]; examples: string[] }
export interface Message { role: string; text: string; ts: number }
export interface Status { provider_ready: boolean; provider_problem: string | null; model: string | null; grants: Grant[]; audit_intact: boolean }
export interface AuditEntry { seq: number; ts: number; session: string; actor: string; kind: string; tool: string; args_digest: string; decision: string; detail: string; hash: string }
export interface Activity { entries: AuditEntry[]; intact: boolean; broken_at: number | null }

/** Events streamed from Rust on `coworker:event`. */
export type CoworkerEvent = { session: string } & (
  | { type: "status"; state: "running" | "idle" }
  | { type: "text"; text: string }
  | { type: "reset" }
  | { type: "tool_started"; tool: string; call_id: string; arguments: unknown }
  | { type: "tool_finished"; tool: string; call_id: string }
  | { type: "approval_required"; id: string; tool: string; arguments: unknown; reason: string; standing_ok: boolean; hard_floor: boolean }
  | { type: "approval_resolved"; id: string; decision: Decision }
  | { type: "denied"; tool: string; reason: string }
  | { type: "question"; id: string; question: string; options: string[] }
  | { type: "provider_error"; category: string; retryable: boolean }
  | { type: "usage"; input_tokens: number; output_tokens: number; session_input: number; session_output: number }
  | { type: "done"; text: string }
  | { type: "cancelled" }
  | { type: "error"; message: string }
);

export const cw = {
  status: () => invoke<Status>("coworker_status"),
  addGrant: (path?: string) => invoke<Grant | null>("coworker_add_grant", { path: path ?? null }),
  revokeGrant: (id: number) => invoke<void>("coworker_revoke_grant", { id }),
  sessions: () => invoke<SessionRow[]>("coworker_sessions"),
  messages: (session: string) => invoke<Message[]>("coworker_messages", { session }),
  newSession: (specialist?: string) => invoke<SessionRow>("coworker_new_session", { specialist: specialist ?? null }),
  specialists: () => invoke<Specialist[]>("coworker_specialists"),
  send: (session: string, text: string) => invoke<void>("coworker_send", { session, text }),
  decide: (id: string, decision: Decision) => invoke<boolean>("coworker_decide", { id, decision }),
  answer: (id: string, text: string) => invoke<boolean>("coworker_answer", { id, text }),
  cancel: (session: string) => invoke<void>("coworker_cancel", { session }),
  activity: (limit?: number) => invoke<Activity>("coworker_activity", { limit: limit ?? null }),
  pending: () => invoke<{ id: string; session: string; kind: string; tool: string }[]>("coworker_pending"),
  resume: (session: string) => invoke<boolean>("coworker_resume", { session }),
  usage: (session?: string) => invoke<{ input_tokens: number; output_tokens: number }>("coworker_usage", { session: session ?? null }),
};
