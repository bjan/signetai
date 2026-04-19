---
title: "Connector: Py Agent"
id: connector-py-agent
status: planning
informed_by: []
section: "Connectors"
depends_on:
  - "signet-runtime"
success_criteria:
  - "Py Agent connector installs and syncs Signet memory lifecycle hooks"
scope_boundary: "Installs hooks and identity files into PyAgent's config; does not define new daemon endpoints or modify the memory pipeline"
draft_quality: "auto-generated, needs user validation before implementation"
---

# Connector: Py Agent

Spec metadata:
- ID: `connector-py-agent`
- Status: `planning`
- Hard depends on: `signet-runtime`
- Registry: `docs/specs/INDEX.md`

Related docs:
- `libs/connector-base/src/index.ts` (BaseConnector contract)
- `integrations/claude-code/connector/src/index.ts` (reference connector)
- `docs/specs/approved/signet-runtime.md`

---

## 1) Problem

Signet has TypeScript connectors for Claude Code, OpenClaw, OpenCode, and Codex,
but no connector for Python-based agent frameworks. PyAgent is a Python agent
runtime with its own config file, lifecycle hooks, and tool-calling conventions.
Python-native agents cannot participate in the Signet memory lifecycle without
manually calling the daemon HTTP API. This means no automatic session-start
injection, no transcript persistence, and no memory extraction for Python agents.

## 2) Goals

1. Ship a `@signet/connector-py-agent` package extending `BaseConnector`.
2. Install hook commands into PyAgent's config file (`~/.pyagent/config.yaml`).
3. Sync identity files (AGENTS.md, SOUL.md, IDENTITY.md, USER.md, MEMORY.md)
   into PyAgent's identity directory.
4. Register Signet as an MCP server in PyAgent's tool registry.
5. Maintain idempotent install/uninstall matching the existing connector pattern.

## 3) Proposed capability set

### A) BaseConnector subclass

Create `integrations/py-agent/connector/src/index.ts` extending `BaseConnector`.
Implement the four abstract methods:
- `install(basePath)` — patch PyAgent's YAML config with hook commands,
  compose identity extras via `composeIdentityExtras()`, symlink skills.
- `uninstall()` — remove Signet hooks from config, clean generated files.
- `isInstalled()` — check for Signet hook presence in PyAgent config.
- `getConfigPath()` — return `~/.pyagent/config.yaml`.

Hook commands follow the same pattern as Claude Code: shell commands calling
`signet hook session-start -H pyagent --project "$(pwd)"` with appropriate
timeouts.

### B) Hook lifecycle mapping

Map PyAgent's lifecycle events to Signet's daemon hook API:

| PyAgent event | Signet hook | Daemon endpoint |
|---|---|---|
| `on_session_start` | session-start | `POST /api/hooks/session-start` |
| `on_user_message` | user-prompt-submit | `POST /api/hooks/user-prompt-submit` |
| `on_session_end` | session-end | `POST /api/hooks/session-end` |

PyAgent does not have a pre-compaction event. The connector omits that hook.
The `x-signet-runtime-path` header is set to `"legacy"` since PyAgent does
not run a Signet plugin natively.

### C) Identity file sync

On install, compose identity content from `~/.agents/` and write a generated
`AGENTS.md` into PyAgent's identity directory (`~/.pyagent/identity/AGENTS.md`).
Include the Signet system block via `buildSignetBlock()`. On file-watcher
sync events from the daemon, the connector re-generates the composed file.

### D) MCP server registration

Register Signet's MCP server in PyAgent's tool config so that agents running
under PyAgent can call `memory_search`, `memory_store`, `knowledge_expand`,
and other Signet MCP tools. Config entry points to `signet-mcp` stdio binary.

### E) CLI integration

Register the connector in the CLI setup wizard's harness selector. Add
`pyagent` to the harness ID enum and the `signet setup` interactive flow.

## 4) Non-goals

- No Python SDK or pip-installable package (use HTTP API directly).
- No modification to PyAgent's source code or runtime.
- No new daemon endpoints; reuse existing hook API.
- No plugin-path runtime (legacy path only until PyAgent adds plugin support).

## 5) Integration contracts

### Connector <-> Daemon

- All hook calls include `harness: "pyagent"` and `agent_id` from config.
- Session tracker accepts `"pyagent"` as a valid harness identifier.
- Runtime path is `"legacy"` — no plugin mutex conflict.

### Connector <-> CLI

- `signet setup` lists PyAgent alongside existing harnesses.
- `signet status` shows PyAgent connector state when installed.

### Connector <-> Multi-Agent

- Agent scoping works via standard `agent_id` parameter on hook payloads.
- No special handling needed beyond what existing connectors provide.

## 6) Rollout phases

### Phase 1: Core connector

Implement the `BaseConnector` subclass with install, uninstall, hooks
configuration, and identity sync. Add to CLI harness selector. Verify
against PyAgent's config schema with integration tests.

### Phase 2: MCP and polish

Register MCP server in PyAgent's tool config. Add Windows support for
command paths. Publish package to npm. Write setup wizard copy.

## 7) Validation and tests

- Unit tests for YAML config patching (add hooks, remove hooks, idempotent).
- Unit tests for identity file composition and Signet block injection.
- Integration test: install, verify `isInstalled()` returns true, uninstall,
  verify hooks removed.
- Agent scoping test: hook payloads include correct `agent_id`.
- Regression test: install over existing user config preserves non-Signet keys.

## 8) Success metrics

- PyAgent connector installs cleanly on Linux and macOS.
- Session-start injection returns identity and memories within 3s timeout.
- Memory extraction fires on session-end without errors.
- Repeated install/uninstall cycles leave config in clean state.

## 9) Open decisions

1. PyAgent's exact config schema and lifecycle event names need confirmation
   from reference documentation or a running instance.
2. Whether PyAgent supports MCP natively or requires a tool-adapter shim.
3. Whether to support PyAgent's sub-agent spawning for context inheritance
   (defer to sub-agent-context-continuity spec if so).
