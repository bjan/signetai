---
title: "Connector: Hermes Agent"
id: connector-hermes-agent
status: planning
informed_by: []
section: "Connectors"
depends_on:
  - "signet-runtime"
success_criteria:
  - "Hermes Agent connector installs and syncs Signet memory lifecycle hooks"
scope_boundary: "Bridges Hermes lifecycle events and memory to Signet daemon; does not replace Hermes's native memory system or modify its tool registry internals"
draft_quality: "auto-generated, needs user validation before implementation"
---

# Connector: Hermes Agent

Spec metadata:
- ID: `connector-hermes-agent`
- Status: `planning`
- Hard depends on: `signet-runtime`
- Registry: `docs/specs/INDEX.md`

Related docs:
- `libs/connector-base/src/index.ts` (BaseConnector contract)
- `references/hermes-agent/` (Hermes source reference)
- `docs/research/technical/RESEARCH-COMPETITIVE-SYSTEMS.md`

---

## 1) Problem

Hermes Agent (Nous Research) is a Python/JS agent runtime with 90+ tools, a
multi-platform gateway (Telegram, Discord, Slack, WhatsApp, Signal), skill
learning, and built-in memory (FTS5 session search + Honcho user modeling).
Its memory is siloed per platform — Telegram sessions cannot share learned
context with CLI sessions beyond basic FTS recall.

Signet's structured memory (entity graph, decay, contradiction detection,
constraints) complements Hermes's session-level memory. The connector bridges
lifecycle events to Signet without replacing native memory, creating a
dual-memory architecture: Signet for long-term structured persistence,
Hermes for session-local recall.

## 2) Goals

1. Ship `@signet/connector-hermes-agent` extending `BaseConnector`.
2. Bridge session lifecycle (start, message, end) to Signet daemon hooks.
3. Sync identity files into Hermes's personality system.
4. Route memories from all gateway platforms through the same Signet agent.
5. Preserve Hermes's native memory as the session-local layer.

## 3) Proposed capability set

### A) BaseConnector subclass

`integrations/hermes-agent/connector/src/index.ts` extending `BaseConnector`.
Hermes data dir: `~/.hermes/`, config: `cli-config.yaml`.

- `install(basePath)` — patch config with hooks, compose identity, symlink skills.
- `uninstall()` — remove Signet entries from config.
- `isInstalled()` — check for Signet integration key.
- `getConfigPath()` — `~/.hermes/cli-config.yaml`.

### B) Hook lifecycle mapping

| Hermes event | Signet hook | Notes |
|---|---|---|
| Session start | session-start | Inject context alongside personality |
| User message | user-prompt-submit | Each turn, all platforms |
| Session end / reset | session-end | Extract memories from transcript |
| `/compress` | pre-compaction | Manual context compression |

Harness field: `"hermes-agent"`. Platform (telegram, discord, cli) passed as
`metadata.platform`. Runtime path: `"legacy"`.

### C) Gateway-aware session routing

Single gateway serves multiple platforms. Connector maps to one Signet session
key per user: `hermes:{agent}:{user_id}`. Memory from Telegram is retrievable
in Discord. Agent scoping via `agent_id` from Signet config.

### D) Dual-memory architecture

Hermes native memory runs unmodified. Signet operates as additional layer:
session-start injects structured memories alongside personality prompt;
session-end extracts via pipeline. No deduplication — both systems serve
different retrieval patterns.

### E) Identity sync

Generate `signet.md` personality file at `~/.hermes/personalities/` composed
from AGENTS.md + SOUL.md + IDENTITY.md + Signet system block. User activates
via `hermes personality signet` or config default.

## 4) Non-goals

- No replacement of Hermes native memory or skill learning.
- No modification to Hermes tool registry or gateway internals.
- No Hermes-side plugin (config patching only).
- No multi-agent within single Hermes instance.

## 5) Integration contracts

**Connector <-> Daemon**: `harness: "hermes-agent"`, `metadata.platform`.
Session key `hermes:{agent}:{user}`. Legacy runtime path.

**Connector <-> Multi-Agent**: `agent_id` scopes all operations. Platform
metadata is informational, not a scoping dimension.

**Connector <-> Knowledge Graph**: Hermes skill events could feed entity graph
as `skill` entities. Deferred to adaptive-skill-lifecycle spec.

## 6) Rollout phases

### Phase 1: CLI-only connector
BaseConnector subclass for Hermes CLI. Hook install, identity sync, session
lifecycle. Validate against running Hermes instance.

### Phase 2: Gateway integration
Cover gateway events (Telegram, Discord, Slack). Cross-platform session key
routing. Test: store from CLI, retrieve from Telegram.

### Phase 3: Skill bridge (deferred)
Bridge Hermes skill events to Signet procedural memory. Blocked on
procedural-memory-plan and adaptive-skill-lifecycle.

## 7) Validation and tests

- YAML config patching (add, remove, idempotent).
- Session key generation from platform + user identity.
- Integration: install, verify hooks in config.
- Gateway: two platforms produce memories under same agent.
- Identity: personality file contains composed Signet identity.
- Agent scoping: hook payloads include correct `agent_id`.

## 8) Success metrics

- Installs cleanly via `signet setup`.
- Context injection works in CLI and at least one gateway platform.
- Telegram memories retrievable in subsequent CLI session.
- Idempotent install/uninstall preserves existing Hermes config.

## 9) Open decisions

1. Whether config patching suffices or gateway events need runtime adapter.
2. Session key format for gateway users without stable cross-platform IDs.
3. Whether to expose Signet MCP tools through Hermes's tool registry.
