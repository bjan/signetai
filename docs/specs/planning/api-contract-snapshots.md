---
title: "API Contract Snapshot Testing"
id: api-contract-snapshots
status: planning
informed_by: []
section: "DevEx"
depends_on:
  - "signet-runtime"
success_criteria:
  - "Critical API response contracts are snapshotted to detect accidental breaking changes"
scope_boundary: "Test files, snapshot fixtures, and a CI step. No changes to daemon route handlers unless a snapshot reveals a needed fix."
draft_quality: "auto-generated, needs user validation before implementation"
---

# API Contract Snapshot Testing

## Problem

The daemon HTTP API (Hono, port 3850) has ~40 routes documented in
docs/API.md. Response shape changes can break the SDK, connectors,
dashboard, and CLI consumers. Currently, nothing enforces that a
response shape stays stable across commits. Breaking changes are
caught only by manual review or downstream test failures.

## Goals

1. Snapshot the response shape (JSON structure, status codes, headers)
   of critical API routes.
2. CI detects accidental response shape changes and fails the build.
3. Intentional changes require an explicit snapshot update.

## Non-goals

- Testing response values or business logic (covered by unit tests).
- Load testing or performance benchmarks.
- Snapshotting every route (focus on high-traffic consumer contracts).

## Proposed approach

**Snapshot test file** (`platform/daemon/src/__tests__/api-contracts.test.ts`):
A bun test file that starts a daemon instance (or uses the test
helper), sends requests to each critical route, and snapshots the
response shape. Use bun's built-in snapshot testing (`expect().toMatchSnapshot()`).

**Shape extraction**: For each response, extract a normalized shape:
strip variable values (IDs, timestamps, counts), keep keys, types,
and structure. A helper function `extractShape(obj)` recursively maps
values to their type strings (`"string"`, `"number"`, `"array"`, etc.).

**Critical routes** (initial set):
- `GET /health`
- `GET /api/config`
- `POST /memory/search`
- `POST /api/remember`
- `GET /api/memories`
- `GET /api/skills`
- `GET /api/sessions`

**Snapshot files**: Stored in `platform/daemon/src/__tests__/__snapshots__/`.
Checked into git. Updated via `bun test --update-snapshots` when
changes are intentional.

**CI integration**: The API contract test runs as part of the daemon
package test lane. Snapshot mismatches cause a clear failure message
showing the before/after shape diff.

## Phases

### Phase 1 -- Core route snapshots

- Implement shape extraction helper.
- Write snapshot tests for the 7 critical routes listed above.
- Verify snapshots pass on current main.
- Add to CI as part of daemon test lane.

### Phase 2 -- Coverage expansion and docs sync

- Expand to remaining API routes in docs/API.md.
- Add a CI check that flags new routes in source that lack a
  corresponding snapshot test.
- Verify docs/API.md response examples stay consistent with
  snapshots.

## Validation criteria

- Changing a response key name in a snapshotted route fails CI.
- Adding a new optional field to a response updates the snapshot
  (requires explicit `--update-snapshots`).
- Shape extraction correctly normalizes variable values to type
  strings.

## Open decisions

1. Should snapshots capture HTTP headers (Content-Type, caching) or
   only the JSON body shape?
