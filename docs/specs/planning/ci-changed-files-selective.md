---
title: "CI Changed-Files Selective Pipelines"
id: ci-changed-files-selective
status: planning
informed_by: []
section: "DevEx"
depends_on:
  - "memory-pipeline-v2"
success_criteria:
  - "PR CI runs only affected package lanes plus dependency closure while nightly runs full suite"
scope_boundary: "GitHub Actions workflow changes and a workspace dependency graph script. No changes to package source code or build scripts."
draft_quality: "auto-generated, needs user validation before implementation"
---

# CI Changed-Files Selective Pipelines

## Problem

Every PR currently runs the full build/test/typecheck suite across all
~15 packages. Most PRs touch 1-3 packages. This wastes CI minutes,
slows feedback loops, and creates noise when unrelated packages fail.

## Goals

1. PR CI runs only affected packages plus their downstream dependents.
2. Nightly CI runs the full suite as a safety net.
3. Dependency closure is computed from the workspace graph, not hardcoded.

## Non-goals

- Caching build artifacts across runs (separate optimization).
- Parallelizing individual package test suites.

## Proposed approach

**Dependency graph script** (`scripts/ci-affected.ts`): Read the root
workspace globs and `repo.map.yaml`, then parse each discovered
`package.json` for `dependencies` and `devDependencies` referencing
workspace packages. Build an adjacency list. Given a set of changed
paths, resolve to workspace package names, then walk forward edges to
collect the full closure. Output a JSON array of affected package
directories.

**Changed-file detection**: Use `git diff --name-only origin/main...HEAD`
in CI. Map each changed path to its owning workspace package via the
resolved workspace directory prefixes. Root-level changes (`package.json`,
`bunfig.toml`, `tsconfig`, `turbo.json`, `repo.map.yaml`) trigger the full
suite as a safeguard.

**GitHub Actions matrix**: The PR workflow calls the affected script,
then feeds the output into a dynamic matrix for build, test, and
typecheck jobs. Empty matrix (no affected packages) skips those jobs.

**Nightly full run**: A separate scheduled workflow runs all packages
unconditionally on a cron schedule.

## Phases

### Phase 1 -- Affected-packages script and PR workflow

- Implement `scripts/ci-affected.ts` with workspace graph resolution.
- Add unit tests for graph closure (diamond dependencies, root changes).
- Update `.github/workflows/pr.yml` to use dynamic matrix from script output.
- Keep existing full-suite workflow as nightly cron.

### Phase 2 -- Validation and tuning

- Run both selective and full suite in parallel for 2 weeks to verify
  the selective pipeline never misses a real failure.
- Add a CI step that logs skipped packages for auditability.
- Remove parallel full-suite run once confidence is established.

## Validation criteria

- PR touching only `surfaces/cli/` does not run daemon or core tests.
- PR touching `platform/core/` runs core + all downstream packages.
- Root-level changes trigger full suite.
- Nightly cron runs all packages regardless of recent changes.

## Open decisions

1. Should `docs/` or `web/` changes skip all package CI entirely, or
   run a minimal lint-only pass?
2. Should the affected script live in `scripts/` or be an inline
   composite action in `.github/`?
