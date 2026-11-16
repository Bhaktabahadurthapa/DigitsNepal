# 3 Tool calling and registry · 4 MCP integration

## 3. Tool calling and tool registry

- **Description**: typed tools the model may call; each has a schema and a risk class that drives approval.
- **Desktop**: `ToolSpec { name, schema, func, metadata }` and `ToolRegistry.register/execute/clone(exclude)` (`coworker/tools/registry.py:16-79`); schemas derived from function signatures or overridden. Built-ins in `coworker/tools/`: files, search, shell, git, network, plan, todo, ask, directories, subagent, agent_tasks, connreq, toolreq.
- **Risk classes** (`coworker/risk.py`): `READ` (always allowed), `EGRESS` (network; URL can carry data out), `WRITE_LOCAL` (path-scoped, mode-gated), `EXEC` (commands), `EXTERNAL` (off-machine side effects). Unknown tools fall back to metadata (`requires_approval` → external) or read.
- **Harness**: `Tool::new(name).description().input_schema().handler()` registered per namespace (`harness-sdk/src/application_tools.rs`); qualified names `namespace.tool`, model alias `namespace__tool` (`tools-sdk/src/lib.rs:279`); `permissions::Category { Read, WriteLocal, Execute, Egress, External }` (`runtime/src/permissions.rs:52`); argument validation against schema (`runtime.rs:4233`).
- **Security**: only first-party pinned tools are trusted by name; third-party `readOnlyHint` is not trusted (`permissions.rs:60`).
- **Complexity**: Medium. **Confidence**: High.

## 4. MCP integration

- **Desktop**: `MCPManager` (`coworker/mcp/client.py`) over the official `mcp` SDK; one asyncio task per server; stdio and streamable HTTP transports; OAuth (`mcp/oauth.py`); bridged into the sync registry (`mcp/tools.py`).
- **Harness**: `McpServer::stdio(name, absolute_program, args)` / `streamable_http(name, url)` (`harness-sdk/src/mcp.rs`); program must be an absolute path; identity immutable per name; at most 16 servers; tools selected by qualified name in the agent definition.
- **Security**: MCP tools run with host privileges unless sandboxed; annotations are hints only.
- **Complexity**: Medium. **Confidence**: High.
