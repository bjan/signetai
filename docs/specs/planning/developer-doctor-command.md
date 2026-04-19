---
title: "Developer Doctor Command"
id: developer-doctor-command
status: planning
informed_by: []
section: "DevEx"
depends_on:
  - "signet-runtime"
success_criteria:
  - "A single doctor command validates local setup, daemon health, and baseline checks"
scope_boundary: "A new CLI command and supporting check modules. No changes to daemon, core, or build infrastructure."
draft_quality: "auto-generated, needs user validation before implementation"
---

# Developer Doctor Command

## Problem

New contributors and even experienced developers frequently debug
setup issues manually: wrong bun version, daemon not running, port
conflict, stale migrations, missing native module build. There is no
single command that validates the local development environment and
reports what needs fixing.

## Goals

1. One command checks all prerequisites and reports pass/fail for
   each.
2. Output is human-readable with actionable fix suggestions.
3. Runs without requiring a working build (checks prerequisites for
   building).

## Non-goals

- Auto-fixing issues (doctor reports, human fixes).
- Replacing CI checks (this is a local development tool).

## Proposed approach

**Command**: `signet doctor` (via CLI package) and `bun run doctor`
(via root package.json script pointing to `scripts/doctor.ts`). Both
entry points call the same check runner.

**Check modules** (each returns `{ name, status, message }`):

1. **bun version**: Verify bun >= 1.1. Parse `bun --version`.
2. **node version**: Verify node >= 20 if present (some packages
   target node).
3. **dependencies installed**: Check `node_modules/` exists and
   `bun install --frozen-lockfile` would succeed.
4. **build artifacts**: Verify core, connector-base, and daemon have
   built dist/ directories.
5. **daemon running**: HTTP GET to `http://127.0.0.1:3850/health`.
   Report running/stopped/port-conflict.
6. **SQLite OK**: Open the memory database, run a simple query.
   Report corruption or missing file.
7. **migrations current**: Compare applied migration count against
   available migration files in `platform/core/src/migrations/`.
8. **disk space**: Check available space on the partition containing
   `$SIGNET_WORKSPACE`. Warn if < 1GB.
9. **port available**: If daemon is not running, check that port 3850
   is not occupied by another process.
10. **git remote**: Verify a git remote is configured for
    `$SIGNET_WORKSPACE` if git sync is enabled.

**Output format**: Each check prints a status line:
`[PASS] bun version 1.1.45`, `[FAIL] daemon not running -- run: signet start`,
`[WARN] disk space low (800MB free)`. Exit code is non-zero if any
check fails.

## Phases

### Phase 1 -- Core checks

- Implement check runner and checks 1-7.
- Add `signet doctor` command to CLI package.
- Add `bun run doctor` script to root package.json.

### Phase 2 -- Environment checks and polish

- Add checks 8-10 (disk, port, git remote).
- Add `--json` flag for machine-readable output.
- Add `--fix` flag that runs safe auto-fixes (bun install, build).

## Validation criteria

- Running `signet doctor` with daemon stopped reports the daemon
  check as FAIL with a fix suggestion.
- Running with all prerequisites met reports all PASS and exits 0.
- Missing bun reports FAIL with minimum version requirement.

## Open decisions

1. Should `--fix` auto-run `bun install` and `bun run build`, or
   just print the commands to run?
