# 10 Sub-agents · 11 Multi-agent teams · 12 Scheduling and automations

## 10. Sub-agents

- **Desktop**: `explore` tool (`coworker/tools/subagent.py`) spawns a child `TurnEngine` over the same workspace with read-only tools, a fresh context, plan mode (writes/shell hard-blocked) and no approver; only its final report returns. `_CHILD_MAX_ITERATIONS = 10`. No recursion: the child registry has no `explore` tool.
- **Harness**: `subagents::Config { enabled, max_concurrent: 4, max_children: 32, limits }` (`runtime/src/subagents.rs:24`); `Agent::subagent(..)`, `allow_subagents(true)`; child events surface as `RunEvent::SubagentActivity`.
- **Security**: depth limit by construction, children inherit permission checks.
- **Complexity**: Medium. **Confidence**: High.

## 11. Multi-agent teams

- **Desktop**: board as a projection of an append-only team event log (`coworker/teams/{model,store,journal}.py`); `ItemState { OPEN, IN_PROGRESS, BLOCKED, REVIEW, DONE, CANCELED }`; team proposals need user approval (`TEAM_PROPOSED` event); built-in personas (`coworker/personas/builtin`: swe-lead, swe-worker, test-worker, reviewer, appsec, devsecops-lead, …).
- **Harness**: `AgentTeam`, `Team::{create_item, assign, transition, post_message, cancel}`; SQLite tables `teams`, `team_members`, `team_queue`, `team_events`, `team_commands` (`runtime/src/store.rs`); instructions tell members that board messages are "coordination, not human authorization" and workers must request lead review (`runtime.rs:671-678`).
- **Security**: agent-to-agent messages never count as approval.
- **Complexity**: High. **Confidence**: Medium (models and rules read; full orchestration not traced).

## 12. Scheduling and recurring automations

- **Desktop**: `Scheduler` loop in the always-on server (`coworker/automation/scheduler.py`): run-once catch-up for missed runs, skip-on-overlap. `ScheduledTask { title, instructions, schedule (cron | once), workspace, agent, model, notify_on_completion, always_allowed_tools, always_allowed_commands }` and `TaskRun { task_id, run_id, started_at, finished_at }` (`automation/models.py:119-241`); SQLite `scheduled_tasks`, `task_runs` (`automation/store.py:86-92`). Unattended runs park approvals in the Inbox.
- **Harness**: `Agent::scheduling(true)` exposes `schedule_create/list/update/cancel/delete` built-ins (`runtime/src/assistant_tools.rs:46`); `schedule_list` is read-category.
- **Security**: per-task allowlists are explicit grants; unattended runs never self-approve.
- **Complexity**: Medium. **Confidence**: High (Desktop), Medium (Harness).
