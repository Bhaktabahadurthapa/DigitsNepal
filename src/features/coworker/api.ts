// Typed wrappers for the Sahayogi and Activity commands.
import { invoke } from "@tauri-apps/api/core";

export type Decision = "once" | "session" | "deny";
export interface Grant { id: number; path: string; created_at: number }
export interface SessionRow { id: string; agent: string; specialist: string; title: string; created_at: number; updated_at: number }
export interface Specialist { key: string; name: string; nepali: string; summary: string; icon: string; version: number; tools: string[]; examples: string[] }
export interface McpTool { name: string; description: string; input_schema: unknown; read_only: boolean; destructive: boolean }
export interface McpServer { name: string; url: string; tools: McpTool[]; enabled: string[]; created_at: number }
export interface Automation { id: number; name: string; description: string; steps: { tool: string; args: Record<string, unknown>; note?: string }[]; created_at: number; last_run_at: number | null; runs: number }
export interface Security { paused: boolean; network_off: boolean; code_enabled: boolean }
export interface SecurityStatus extends Security { sandbox_available: boolean; python: string | null; javascript: string | null }
export interface Standing { session: string; tool: string; created_at: number; title: string }
export interface PendingItem { id: string; session: string; kind: string; tool: string; preview: unknown; reason: string; standing_ok: boolean; hard_floor: boolean; created_at: number; live: boolean }
export interface IntentHints { language: string; category: string; confidence: number; alternatives: string[]; actions: string[]; sensitive_actions: string[]; dates: { text: string; date: string }[]; times: string[]; amounts: { text: string; amount: number; currency: string }[]; locations: string[]; preferences: string[] }
export interface TaskPreview { hints: IntentHints; workflow: string | null; stop_at: string | null }
export interface PlanStep { title: string; tool: string; needs_approval: boolean; status: "pending" | "running" | "done" | "skipped" | "failed"; note: string }
export interface TaskRow {
  id: string; session: string; specialist: string; request: string; intent: IntentHints;
  plan: { summary?: string; steps?: PlanStep[]; warnings?: string[]; missing_info?: string[] };
  evidence: { title: string; source: string }[];
  result: { outcome?: string; summary?: string; verified?: boolean; actions?: { tool: string; target: string; verified: boolean; why: string }[]; next_steps?: string[] };
  state: string; state_reason: string; paused: boolean; created_at: number; updated_at: number;
}
export interface TaskDetail { task: TaskRow; events: { ts: number; kind: string; from: string; to: string; detail: unknown }[]; input_tokens: number; output_tokens: number; interventions: number }
export interface TaskMetrics { total: number; completed: number; verified: number; failed: number; cancelled: number; active: number; completion_rate: number | null; verified_rate: number | null; interventions_per_task: number | null; tokens_per_task: number | null; median_minutes: number | null }
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
  mcpServers: () => invoke<McpServer[]>("coworker_mcp_servers"),
  mcpAdd: (name: string, url: string) => invoke<McpServer>("coworker_mcp_add", { name, url }),
  mcpEnable: (name: string, tools: string[]) => invoke<void>("coworker_mcp_enable", { name, tools }),
  mcpRemove: (name: string) => invoke<void>("coworker_mcp_remove", { name }),
  automations: () => invoke<Automation[]>("coworker_automations"),
  deleteAutomation: (id: number) => invoke<void>("coworker_automation_delete", { id }),
  send: (session: string, text: string) => invoke<void>("coworker_send", { session, text }),
  decide: (id: string, decision: Decision) => invoke<boolean>("coworker_decide", { id, decision }),
  answer: (id: string, text: string) => invoke<boolean>("coworker_answer", { id, text }),
  cancel: (session: string) => invoke<void>("coworker_cancel", { session }),
  activity: (limit?: number) => invoke<Activity>("coworker_activity", { limit: limit ?? null }),
  pending: () => invoke<PendingItem[]>("coworker_pending"),
  security: () => invoke<SecurityStatus>("coworker_security"),
  taskPreview: (text: string) => invoke<TaskPreview>("coworker_task_preview", { text }),
  createTask: (text: string, specialist?: string) => invoke<TaskRow>("coworker_task_create", { text, specialist: specialist ?? null }),
  tasks: () => invoke<TaskRow[]>("coworker_tasks"),
  task: (id: string) => invoke<TaskDetail>("coworker_task", { id }),
  taskControl: (id: string, action: "pause" | "resume" | "cancel" | "retry") => invoke<void>("coworker_task_control", { id, action }),
  taskMetrics: () => invoke<TaskMetrics>("coworker_task_metrics"),
  setSecurity: (switches: Security) => invoke<void>("coworker_set_security", { switches }),
  standing: () => invoke<Standing[]>("coworker_standing"),
  revokeStanding: (session?: string, tool?: string) => invoke<number>("coworker_revoke_standing", { session: session ?? null, tool: tool ?? null }),
  stopAll: () => invoke<number>("coworker_stop_all"),
  resume: (session: string) => invoke<boolean>("coworker_resume", { session }),
  usage: (session?: string) => invoke<{ input_tokens: number; output_tokens: number }>("coworker_usage", { session: session ?? null }),
};
