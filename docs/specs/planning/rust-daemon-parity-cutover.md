---
title: "Rust Daemon Parity and Runtime Cutover"
id: rust-daemon-parity-cutover
status: planning
informed_by: []
section: "Daemon"
depends_on:
  - "daemon-rust-rewrite"
  - "memory-pipeline-v2"
success_criteria:
  - "Rust daemon reaches feature parity with JS daemon and becomes primary runtime"
scope_boundary: "Defines the parity checklist, divergence tracking, and cutover strategy from JS to Rust daemon; does not define new features beyond what the JS daemon already implements"
draft_quality: "auto-generated, needs user validation before implementation"
---

# Rust Daemon Parity and Runtime Cutover

Spec metadata:
- ID: `rust-daemon-parity-cutover`
- Status: `planning`
- Hard depends on: `daemon-rust-rewrite`, `memory-pipeline-v2`
- Registry: `docs/specs/INDEX.md`

Related docs:
- `docs/specs/planning/daemon-rust-rewrite.md` (phased rewrite plan)
- `platform/daemon-rs/` (Rust daemon shadow implementation)

---

## 1) Problem

The Rust daemon (`platform/daemon-rs/`) is designed to shadow the JS daemon
and log divergences to `$SIGNET_WORKSPACE/.daemon/logs/shadow-divergences.jsonl`.
The `daemon-rust-rewrite` spec defines the six-phase implementation plan
(proxy -> read endpoints -> writes -> pipeline -> everything else -> cutover).
What it does not define is: how to measure parity, what the acceptance gates
are for cutover, and how to execute the transition without breaking production
deployments.

Key parity risks: (1) migration schema drift between 39+ JS migrations and
Rust `include_str!` SQL, (2) API contract drift across 100+ endpoints
(Hono vs axum serialization), (3) pipeline behavioral drift in LLM-dependent
extraction, (4) secrets backward compat (XSalsa20-Poly1305 + BLAKE2b).

## 2) Goals

1. Define a machine-verifiable parity checklist for each daemon domain.
2. Establish shadow mode as the pre-cutover validation gate.
3. Plan the cutover sequence so no production deployment loses data.
4. Maintain rollback capability for at least one release cycle post-cutover.
5. Preserve the CLAUDE.md Rust parity rule until cutover is complete.

## 3) Proposed capability set

### A) Parity checklist by domain

Per-domain matrix (feature / JS behavior / Rust status). Nine domains:
schema (migration output), read endpoints, write endpoints, pipeline
workers, auth, file watcher, git sync, MCP, and secrets. Each domain
verified independently before cutover gate.

### B) Shadow divergence analysis

Structured divergence classification added to existing shadow logs:
**structural** (JSON shape mismatch), **semantic** (value drift),
**timing** (>2x or >100ms delta). `signet shadow report` aggregates
per-domain summaries. Cutover blocked until structural divergences
reach zero.

### C) Cutover sequence

Shadow mode minimum 2 weeks. Zero structural divergences. Semantic
divergences documented. Rust promoted via `signet start --runtime rust`.
JS fallback retained one release cycle. Default switches to Rust. JS
removed after clean cycle.

### D) Rollback mechanism

`signet start --runtime js` forces JS daemon during transition. Schema
is forward-compatible. Secrets format identical across runtimes.

### E) Migration verification tool

`signet verify-schema` compares `PRAGMA table_info` for every table
between both daemons. Runs in CI on PRs touching either migration set.

## 4) Non-goals

- No new features in the Rust daemon beyond JS parity.
- No performance optimization spec (that comes after cutover).
- No cross-compilation matrix (handled by CI/release infrastructure).
- No changes to the daemon HTTP API contract.

## 5) Integration contracts

### Parity Cutover <-> Daemon Rust Rewrite

- This spec consumes the phased implementation from `daemon-rust-rewrite`.
- Each phase completion triggers the parity checklist for that domain.
- Phase 6 of the rewrite is gated by this spec's cutover criteria.

### Parity Cutover <-> Memory Pipeline V2

- Pipeline stage parity is the highest-risk domain (LLM-dependent behavior).
- Shadow mode must capture extraction decision divergences explicitly.
- Pipeline config (model, thresholds, modes) must be identical across runtimes.

### Parity Cutover <-> Distributed Orchestration

- Cluster protocol must work identically on both runtimes during the
  transition period (mixed-version clusters).

## 6) Rollout phases

### Phase 1 (shadow instrumentation)

- Structured divergence classification in shadow logs.
- `signet shadow report` CLI command.
- Migration verification tool in CI.
- Shadow mode enabled for internal deployments.

### Phase 2 (parity gate)

- Zero structural divergences sustained for 2 weeks.
- Semantic divergences documented per domain.
- Secrets roundtrip verified against production `secrets.enc`.
- `signet start --runtime rust` available as opt-in.

### Phase 3 (cutover)

- Rust daemon becomes default runtime.
- JS fallback retained for one release cycle.
- JS daemon removed from package after clean cycle.

## 7) Validation and tests

- `signet verify-schema` passes on every PR touching migrations.
- Shadow report shows zero structural divergences before cutover.
- Secrets roundtrip byte-identical across runtimes.
- 10 known memories recalled via both runtimes produce identical results.
- Pipeline extraction on canned LLM responses produces identical decisions.
- Rollback from Rust to JS works without data loss.

## 8) Success metrics

- Zero user-reported regressions during cutover.
- Idle memory drops from ~100MB (bun) to ~10MB (Rust).
- Search p99 latency improves at least 2x.
- No secrets lost. Rollback exercised before JS removal.

## 9) Open decisions

1. Whether shadow mode should compare response bodies byte-for-byte or
   use semantic JSON comparison (ignoring field ordering).
2. Minimum shadow duration before cutover (2 weeks proposed, may need longer).
3. Whether to ship Rust binary as optionalDependency in npm package or
   as a separate install channel.
