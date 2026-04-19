---
title: "Golden Path Contributor Docs"
id: golden-path-docs
status: planning
informed_by: []
section: "Docs"
depends_on:
  - "developer-doctor-command"
success_criteria:
  - "Short golden-path guides exist for backend, dashboard, and connector changes"
scope_boundary: "Documentation files in docs/. No code changes."
draft_quality: "auto-generated, needs user validation before implementation"
---

# Golden Path Contributor Docs

## Problem

CLAUDE.md, CONTRIBUTING.md, and ARCHITECTURE.md explain what the
system is but not how to do common tasks step by step. Contributors
waste time figuring out which files to touch, what build order to
follow, and what patterns to replicate. This is especially painful
for agent-driven development where the agent needs a clear recipe.

## Goals

1. Provide step-by-step golden paths for the 5 most common
   contribution types.
2. Each path fits on one screen (~30-40 lines) with concrete file
   paths and commands.
3. Paths stay accurate via CI-checked references to real files.

## Non-goals

- Replacing architecture docs (golden paths reference them, not
  duplicate them).
- Covering every possible contribution type.
- Tutorial-style prose (these are checklists, not narratives).

## Proposed approach

**Location**: `docs/golden-paths/` directory with one file per path.

**Golden paths to write**:

1. **Add an API endpoint** (`add-endpoint.md`): Which file to add
   the route, how to register it in the Hono app, how to add auth
   middleware, how to update docs/API.md, how to add a contract
   snapshot test.
2. **Add a database migration** (`add-migration.md`): Naming
   convention (sequential number), file location in
   `platform/core/src/migrations/`, registration in the index,
   agent_id requirement, idempotency rules, testing approach.
3. **Add a dashboard tab** (`add-dashboard-tab.md`): Svelte 5
   component creation, navigation registration, shadcn-svelte
   component usage, design token reference, build and preview.
4. **Add a connector** (`add-connector.md`): Package scaffold under
   `integrations/<tool>/connector/`, connector-base inheritance, install hook
   implementation, harness config patching, CLI registration.
5. **Add a CLI command** (`add-cli-command.md`): Command file
   location, argument parsing, daemon API integration, help text,
   testing.

**Format**: Each golden path follows a fixed template:
- Prerequisites (what must exist first)
- Steps (numbered, with exact file paths and code patterns)
- Verify (commands to confirm it works)
- Related docs (links to architecture/API docs)

**Freshness**: A CI check (`scripts/ci-golden-paths.ts`) verifies
that file paths referenced in golden paths actually exist. Broken
references fail CI.

## Phases

### Phase 1 -- Core paths

- Write golden paths 1-3 (endpoint, migration, dashboard tab).
- Add freshness check script.
- Link from CONTRIBUTING.md.

### Phase 2 -- Remaining paths

- Write golden paths 4-5 (connector, CLI command).
- Add golden path reference to CLAUDE.md for agent discovery.

## Validation criteria

- A contributor following "Add an API endpoint" produces a working
  route without consulting any other documentation.
- CI fails if a golden path references a file that does not exist.
- Each golden path is under 50 lines.

## Open decisions

1. Should golden paths include copy-pasteable code templates, or
   reference existing files as examples to follow?
