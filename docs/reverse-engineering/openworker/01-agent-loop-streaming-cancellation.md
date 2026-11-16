# 1 Agent loop · 5 Streaming · 16 Errors and cancellation

## 1. Agent execution loop

- **Description**: one user turn spans many model ↔ tool iterations until the model stops calling tools, a limit trips, or the user interrupts.
- **User workflow**: user sends a message → streamed answer and tool activity → approval cards when needed → final answer.
- **Architecture (Desktop)**: `TurnEngine` (`coworker/engine.py:378`) owns the loop. `_loop()` (`engine.py:1345`) checks `max_iterations` (default 12, `engine.py:388`), runs auto-compaction between iterations, streams the model (`_astream`, `engine.py:1844`), then `_handle_tool_calls` (`engine.py:1895`). Low-risk reads run concurrently; writes and shell stay ordered (`_parallel_safe`, `engine.py:2052`). Provider calls are blocking and wrapped in `asyncio.to_thread`.
- **Architecture (Harness)**: `Runtime` (`harness/runtime/src/runtime.rs:62`) with a JSON method dispatcher (`dispatch`, `runtime.rs:1541`: `session/create`, `session/attach`, `session/list`, `policy/resolve`, …). The SDK wraps it as `Harness` → `AgentHandle` → `Session::send/run_streamed` (`harness-sdk/src/agents/session.rs`).
- **APIs / data**: Desktop `Event(EventType, data)`; Harness `Run`, `RunStream { next, finish }`, `RunResult { status, final_output, interruptions, submission_id, cursor, provider_error }` (`session.rs:119`).
- **Limits**: Desktop `max_iterations`, `MAX_TRUNCATION_CONTINUATIONS = 2` (`engine.py:55`). Harness `budget::Limits { max_model_calls: 100, max_output_tokens: 256000, max_output_tokens_per_call }` (`runtime/src/budget.rs:9`).
- **Security boundary**: the loop never executes a tool before the permission decision; tool results are data.
- **Dependencies**: Desktop — aisuite, asyncio. Harness — tokio, reqwest, rusqlite.
- **Complexity**: High (Desktop engine 4,369 lines).
- **Evidence / confidence**: High (source).

## 5. Streaming responses

- **Desktop**: `EventType` (`coworker/events.py:15`): `TURN_START`, `ASSISTANT_DELTA`, `REASONING_DELTA` (display-only, never replayed), `ASSISTANT_MESSAGE`, `TOOL_PROPOSED`, `PERMISSION_REQUIRED`, `QUESTION_REQUESTED`, `TOOL_STARTED/FINISHED`, `TURN_SUMMARY`, `TURN_END`, `ERROR`, `INTERRUPTED`, `COMPACTING/COMPACTED`, `SANDBOX_*`. Partial text is kept without half-formed tool calls (`_partial_turn`, `engine.py:1374`).
- **Harness**: `RunEvent` (`session.rs:159`): `AttemptStarted/Finished`, `TextDelta`, `ReasoningDelta`, `ToolStarted/Finished`, `ApprovalDecision`, `AutoApproved`, `PlanUpdated`, `Interrupted`, `ProviderError`. Docs require "replace, don't append" on retried attempts (`docs/harness/guides/streaming.md`).
- **Transport**: Desktop server over WebSocket with a per-launch token (`X-OpenWorker-Token`, README); Harness in-process channels or stdio.
- **Complexity**: Medium. **Confidence**: High.

## 16. Error handling and cancellation

- **Cancellation (Desktop)**: `request_interrupt()` (`engine.py:668`) works from any state — mid-stream, mid-tool (interrupt hooks kill the command), awaiting approval, between iterations — and gives every pending tool call an error result so history never has orphans.
- **Retry**: `retry()` (`engine.py:1189`) re-runs only when the tail is a retriable error; a context-overflow 400 routes to compaction instead of surfacing.
- **Harness**: `Session::cancel` (`session.rs:936`), `RunStatus::{Completed, Interrupted, Cancelled, Failed}`; `Error::OutcomeUnknown` = "may have committed; do not blindly retry" (`harness-sdk/src/lib.rs:94`).
- **Security**: uncertain outcomes are never auto-retried.
- **Complexity**: Medium. **Confidence**: High.
