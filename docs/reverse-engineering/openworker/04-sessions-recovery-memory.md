# 6 Durable sessions · 7 Session recovery · 13 Memory

## 6. Durable sessions

- **Desktop**: `SessionRecord` (`coworker/sessions.py`); `ConversationStore` (`coworker/conversations.py`) = SQLite index per project + one append-only `conversations/<id>.jsonl` per conversation. Session IDs are validated because they become filenames (path-traversal guard, `conversations.py:23-28`).
- **Harness**: SQLite tables `sessions`, `history(session_id, seq)`, `attempts`, `submissions`, `events` (`runtime/src/store.rs:524-530`, schema version 9); optional `input_attachments`. `SessionRef { session_id, agent, team }`.
- **Agent identity**: agent definitions are immutable; changes need a new agent ID (SDK docs, `create_agent`).
- **Complexity**: Medium. **Confidence**: High.

## 7. Session recovery

- **Desktop**: `TurnEngine.resume()` (`engine.py:1261`) re-processes only the trailing assistant message's unanswered tool calls; prompts that were already answered in the durable wait queue return without re-prompting, so nothing executes twice. `_repair_dangling_tool_calls` (`engine.py:1283`) fixes histories a provider would reject.
- **Harness**: `Harness::resume(SessionRef)`, `Session::resume_streamed()` from the journal cursor; submission IDs allow reconciling after `OutcomeUnknown` (`session.rs:119`, `docs/harness/guides/sessions.md`).
- **Security**: idempotence relies on recorded call IDs, not on re-asking the model.
- **Complexity**: High. **Confidence**: High.

## 13. Memory

- **Desktop**: adapter interface with scopes global / workspace / session (`coworker/memory/base.py`); SQLite backend table `memories` (`memory/sqlite_store.py:25`); memory tools exposed to the agent (`memory/tools.py`).
- **Harness**: `Agent::memory(bool)`, `memory_scope(..)`; `memory_read`/`memory_search` are read-category built-ins (`runtime/src/assistant_tools.rs:57`); table `assistant_memories`.
- **Security**: memory writes are agent-authored and should be visible and deletable by the user.
- **Complexity**: Medium. **Confidence**: Medium.
