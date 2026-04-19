---
title: "Signet Native Harness"
id: signet-native-harness
status: planning
informed_by: []
section: "Runtime"
depends_on:
  - "distributed-harness-orchestration"
  - "signet-runtime"
success_criteria:
  - "Signet ships a first-party harness for controlled benchmarking and production workflows while preserving existing harness support"
scope_boundary: "Defines the first-party execution harness built on the Signet runtime; does not replace third-party connectors or define new LLM provider integrations"
draft_quality: "auto-generated, needs user validation before implementation"
---

# Signet Native Harness

Spec metadata:
- ID: `signet-native-harness`
- Status: `planning`
- Hard depends on: `distributed-harness-orchestration`, `signet-runtime`
- Registry: `docs/specs/INDEX.md`

---

## 1) Problem

Every Signet harness today is a third-party adapter: Claude Code, OpenClaw,
OpenCode, Codex. Each connector translates platform-specific hooks into
daemon API calls, but none is controlled by Signet. When a harness changes
its plugin interface or drops a lifecycle event, Signet loses that surface.

More critically, Signet has no controlled environment for benchmarking
memory quality. LoCoMo experiments (E15-E23) ran on Claude Code with
manual orchestration. There is no repeatable, harness-independent way
to run memory evaluation suites, regression tests, or production
workflows that exercise the full session lifecycle.

The Signet native harness is the reference implementation of the runtime
spec: a tool-calling session loop that talks directly to the daemon API.

## 2) Goals

1. Provide a deterministic execution environment for memory benchmarks.
2. Implement the full session lifecycle (start, prompt, tool dispatch, end).
3. Support multi-provider LLM backends (Ollama, OpenAI, Anthropic).
4. Serve as the canonical reference for what connectors must implement.
5. Run headless for CI and interactive for development.

## 3) Proposed capability set

### A) Session execution loop

A TypeScript process that calls session-start on the daemon, assembles
context (AGENTS.md, MEMORY.md, recalled memories), sends turns to an LLM
provider, dispatches tool calls (MCP tools from daemon + filesystem tools),
and calls session-end with summary. Every step is a daemon API call.
The harness stores no state.

### B) Multi-provider LLM dispatch

Provider trait for Ollama (local, default for benchmarks), OpenAI API,
and Anthropic API. Selection via `agent.yaml` or CLI flag. Streaming
for interactive mode, non-streaming for benchmarks.

### C) Benchmark runner mode

`signet bench run <suite.jsonl>` replays JSONL scenarios, records daemon
responses, and produces hit rates, MRR, and latency percentiles. Isolated
database per run (copy or fresh init) to prevent scope leak.

### D) Interactive CLI mode

`signet chat` starts a REPL session with skills, tool calls, and
automatic session lifecycle management.

### E) Harness contract validation

A test suite any connector can run to verify runtime contract compliance:
session-start payload shape, hook timing, tool dispatch ordering,
session-end summary format. The native harness passes by definition.

## 4) Non-goals

- No GUI or web interface (dashboard already exists).
- No replacement of existing connectors (they remain first-class).
- No custom LLM fine-tuning or training infrastructure.
- No agent scheduling or orchestration (that is distributed-harness-orchestration).

## 5) Integration contracts

### Native Harness <-> Signet Runtime

- The harness is a direct implementation of the runtime spec.
- Every daemon API call follows the runtime interface contract.
- Changes to the runtime spec must update the harness simultaneously.

### Native Harness <-> Distributed Orchestration

- The harness resolves its target daemon via the cluster routing table
  when running in a multi-daemon deployment.
- Agent_id is threaded through all daemon API calls.

### Native Harness <-> Predictive Scorer

- Benchmark runs produce training pairs for the scorer (session context
  + recall outcomes).
- Benchmark isolation prevents training data contamination.

## 6) Rollout phases

### Phase 1 (benchmark-only)

- JSONL scenario runner with isolated database.
- Single LLM provider (Ollama).
- Results output as JSON with hit rates and latency.
- No interactive mode yet.

### Phase 2 (interactive + multi-provider)

- `signet chat` REPL with streaming.
- OpenAI and Anthropic provider support.
- MCP tool dispatch from daemon tool registry.
- Session lifecycle fully managed.

### Phase 3 (contract validation)

- Harness contract test suite published as a package.
- Existing connectors validated against the suite.
- CI lane runs contract tests on every connector change.

## 7) Validation and tests

- Benchmark runner produces identical results across repeated runs (determinism).
- Session lifecycle hooks fire in correct order (start before prompt, end after last turn).
- Agent_id is present on every daemon API call from the harness.
- Benchmark database isolation: production memories.db is never touched.
- Interactive mode handles LLM streaming errors without crashing.
- Contract test suite catches missing hook implementations in connectors.

## 8) Success metrics

- LoCoMo benchmark suite runs unattended in < 10 minutes.
- Benchmark results are reproducible across machines (same scores +/- 1%).
- At least one third-party connector passes the contract validation suite.
- Interactive mode latency overhead < 20ms above raw LLM response time.

## 9) Open decisions

1. Whether the benchmark runner stays under `memorybench/` or gets a CLI-facing wrapper in `surfaces/cli`.
2. How to handle tool-calling loops that exceed a max iteration count.
3. Whether interactive mode supports multi-turn or resets each turn.
