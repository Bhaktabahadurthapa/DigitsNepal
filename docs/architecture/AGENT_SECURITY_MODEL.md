# Agent security model

## Principles

1. **The model never approves itself.** Grants, rules and modes change only through UI commands driven by the user. No tool can write them.
2. **Deny by default.** Unknown tools ask; out-of-scope paths and unauthorised hosts are refused.
3. **One authority.** Every tool call passes `coworker::policy`; handlers re-check scope at execution time.
4. **Content is data.** File, web, MCP and agent-to-agent text never counts as instructions or approval.
5. **Uncertain outcomes are not retried** (engine `OutcomeUnknown`).

## Risk classes (per tool, declared in `ToolSpec`)

| Class | Examples | Default decision |
|---|---|---|
| `read` | files.list/read, translation.*, research.summarize | Allow inside scope |
| `write_local` | files.create, files.move, documents.create | Ask; backup first; standing rule allowed |
| `egress` | browser.search/open, github.* | Ask per host; standing rule per host allowed |
| `exec` | (Technical Agent, future) | Only via Seatbelt runner; ask every time |
| `external` | send email, publish, share, post | **Hard floor**: ask every time, never standing, never auto |

## Hard floors (human-only, any mode)

Sending emails or messages · publishing or sharing · deleting files ·
installing software · privileged commands · financial transactions ·
changing security settings · writing credential files · writing outside
granted folders. There is no delete tool; "move to trash" is a `write_local`
action with backup.

## Permission modes (per session)

| Mode | Behaviour |
|---|---|
| Read-only | Only `read` tools; everything else denied |
| Ask (default) | Policy as above |
| Trusted folder | `write_local` inside one chosen folder runs without asking (revocable rule); floors unchanged |

There is no bypass mode in DigitsNepal.

## Standing rules

Created only from an approval card ("Allow for this chat" / "Always for this
folder or host"). Stored with scope (session, folder, host), creator `user`,
expiry, and revocation time. Listed and revocable in the Approval Center.

## Unattended runs (automations)

Never self-approve. Any `ask` parks in a durable queue shown in the Approval
Center; the run waits or ends with "waiting for you".

## Sandbox requirement

Path validation is not a sandbox. Exec tools and untrusted code run only in a
Seatbelt profile: read/write only the granted folders + a private temp
folder; no home, Keychain, `.ssh`, browser profiles or app state; network only
through an allow-list proxy; no secret-named environment variables; profile
verified from inside before use. Until this exists, no exec tools ship.

## Credentials

Keychain only. The engine receives memory-only credentials or a per-launch
relay token. Keys never enter prompts, files, logs or the audit chain.

## Prompt-injection defences

Tool results marked as untrusted data in instructions; high-risk actions need
approval regardless of who asked; denied calls are not retried; outside-scope
paths refused by policy and again by handlers (tested end to end).

## Audit

Hash-chained ledger: actor (user, policy, agent, system), tool, argument
digest, decision, reason. Integrity shown in Activity. Contents never stored.
