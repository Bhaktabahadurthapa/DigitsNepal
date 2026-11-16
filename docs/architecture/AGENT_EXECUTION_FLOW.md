# Agent execution flow

## One turn

```
User message (Nepali / English)
  │  coworker_send(session, text)
  ▼
service: record message + audit "sent" → session.run_streamed(text)
  ▼
Harness loop ── model call (provider via relay or direct) ──► stream
  │   TextDelta / ToolStarted / ToolFinished → "coworker:event" → UI
  │
  ├─ tool call proposed → engine pauses (Approvals::Always)
  │     ▼
  │   policy::evaluate(tool, args, grants, rules, mode)
  │     ├─ Allow → respond Approve(Once) · audit "policy allow"
  │     ├─ Deny  → respond Deny · audit "policy deny" · UI "Blocked"
  │     └─ Ask   → persist pending_interaction · UI approval card
  │                  user: once / standing rule / deny  (floors: once only)
  │                  audit "user …" → respond → resume_streamed
  │
  ├─ handler runs (re-checks scope, timeout, cancellation) → result as data
  └─ loop until final answer, limit hit, cancel, or failure
  ▼
RunStatus: Completed → save answer, usage row · Failed → error with code ·
Cancelled → pending approvals denied
```

## Limits per turn

`TurnLimits`: max model calls, max output tokens, per-call output cap;
per-tool timeout; Harness iteration budget. Exceeding a limit ends the turn
with a clear message, never silently.

## Restart and recovery

1. On launch, load `pending_interactions` with `resolved_at IS NULL`.
2. Re-show their cards. Resuming a session (`Harness::resume`) replays only
   unanswered calls; answered calls never run twice.
3. Turns whose outcome is unknown are marked "uncertain" and are not retried
   automatically.

## Delegation (Phase 7)

```
Orchestrator (Coworker)
  ├─ plan: tasks[] with agent, inputs, budget
  ├─ delegate → Writer / Research / Document / Technical (depth 1 only)
  │     each child: own session, same policy and ledger, own budget
  ├─ track: tasks table + UI progress
  ├─ validate outputs (format + faithfulness checks for translations)
  └─ approvals still reach the human; children cannot approve for each other
Hard stops: max depth 1, max children per turn, total token budget, cancel
propagates to children.
```

## Automation runs

Scheduler (in-app, while DigitsNepal runs) → creates a session for the
automation's agent → same flow as above. `ask` decisions park in the
Approval Center and the run reports "waiting for you". Missed runs: run once
on next launch; overlapping runs are skipped.

## Error handling

| Failure | Behaviour |
|---|---|
| Provider error | Show category + code; session stays usable (tested) |
| Gateway stream quirk | Relay normalises (tested) |
| Tool error | Returned to the model as data |
| Policy deny | Model told not to retry |
| Engine crash / restart | Resume from journal; pending cards restored |
| Outcome unknown | Marked uncertain; no blind retry |
