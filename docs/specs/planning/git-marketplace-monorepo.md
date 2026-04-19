---
title: "Git Marketplace Monorepo (Skills + MCP Servers)"
id: git-marketplace-monorepo
status: planning
informed_by: []
section: "Marketplace"
depends_on:
  - "predictor-agent-feedback"
success_criteria:
  - "GitHub-authenticated pull-request workflow supports publishing and reviewing skills and MCP servers"
scope_boundary: "Git-native marketplace for publishing and reviewing skills and MCP servers. Does not cover runtime invocation analytics (see mcp-cli-bridge-and-usage-analytics) or skill auto-creation (see adaptive-skill-lifecycle)."
draft_quality: "auto-generated, needs user validation before implementation"
---

# Git Marketplace Monorepo (Skills + MCP Servers)

*npm registry semantics on git-native infrastructure, with PR-based
review and automated quality scoring.*

---

## Problem Statement

Signet skills and MCP servers are currently local-only. There is no
mechanism for sharing, discovering, or reviewing community-contributed
capabilities. Users cannot publish a skill they built, and agents
cannot install a skill another user published. The existing
`/api/skills/install` endpoint handles local filesystem installation
but has no concept of a remote registry.

A git-native marketplace avoids the operational burden of hosting a
package registry while preserving auditability (every change is a
commit), review workflow (PRs), and user-scoped namespacing (directory
per publisher).

---

## Goals

1. Define a monorepo structure where each publisher owns a namespace directory.
2. Automate submission review via a Cloudflare Worker that validates structure, runs safety checks, and generates review JSON artifacts.
3. Support both skills (executable tool definitions) and MCP server packages (server config + metadata).
4. Integrate with the daemon so `signet skill install @user/skill-name` resolves from the marketplace.
5. Feed marketplace quality signals (download count, feedback score) into the predictor.

---

## Proposed Capability Set

### A. Monorepo Structure

Hosted as a public GitHub repository (e.g., `signetai/marketplace`):

```
marketplace/
├── registry.json           # auto-generated index of all packages
├── publishers/
│   ├── nicholai/
│   │   ├── profile.json    # publisher metadata, verified identity
│   │   ├── web-deploy/
│   │   │   ├── manifest.yaml
│   │   │   ├── skill.md
│   │   │   └── tool.ts
│   │   └── github-review/
│   │       ├── manifest.yaml
│   │       └── server.json
│   └── avery/
│       └── tattoo-ref/
│           ├── manifest.yaml
│           └── skill.md
```

Each package directory contains a `manifest.yaml` with:

```yaml
name: web-deploy
type: skill | mcp-server
version: 1.0.0
description: "Deploy static sites to Cloudflare Pages"
tags: [deploy, cloudflare, web]
entrypoint: tool.ts | server.json
min_signet_version: "0.76.0"
```

### B. Submission Workflow

1. Publisher forks the monorepo, adds their package under
   `publishers/{username}/`.
2. Publisher opens a PR against `main`.
3. The reviews Cloudflare Worker (`web/workers/reviews/`) runs automated
   checks on PR creation/update:
   - Manifest schema validation (required fields, valid types).
   - Entrypoint file exists and is parseable.
   - No filesystem escapes (symlinks, `../` references).
   - Version bump required if package directory already exists.
   - Size limits (individual file <100KB, package total <500KB).
4. Worker posts a review comment with pass/fail and generates a
   `review.json` artifact committed to the PR branch.
5. Maintainer approval merges the PR. A post-merge hook regenerates
   `registry.json` from all `manifest.yaml` files.

### C. Registry Index

`registry.json` is the discovery index, regenerated on every merge
to `main` via GitHub Action:

```json
{
  "version": 2,
  "updated_at": "2026-03-24T00:00:00Z",
  "packages": [
    {
      "name": "web-deploy",
      "publisher": "nicholai",
      "type": "skill",
      "version": "1.0.0",
      "tags": ["deploy", "cloudflare"],
      "downloads": 0,
      "score": null
    }
  ]
}
```

The daemon caches this index locally at
`$SIGNET_WORKSPACE/.marketplace/registry.json` with a 1-hour TTL.

### D. Daemon Integration

- `GET /api/marketplace/search?q=deploy&type=skill` — searches
  cached registry by name, tags, description. Returns ranked results.
- `POST /api/marketplace/install` — clones the package from the
  monorepo into `$SIGNET_WORKSPACE/skills/` (skills) or registers
  the MCP server config (servers). Records `agent_id` on install.
- `GET /api/marketplace/installed` — lists locally installed
  marketplace packages with version and update status.
- `POST /api/marketplace/feedback` — records user/agent quality
  rating (1-5) for an installed package. Aggregated scores flow
  back to the registry via periodic sync.

### E. CLI Integration

- `signet marketplace search <query>` — searches the registry.
- `signet marketplace install @publisher/package` — installs.
- `signet marketplace publish` — validates local package and opens
  a PR via `gh pr create` (requires GitHub CLI auth).
- `signet marketplace update` — checks for version bumps on
  installed packages.

### F. Quality Scoring

Package quality score is computed from:
- Automated review pass/fail history.
- Download count (install events across all users, via opt-in
  telemetry ping to a stats endpoint).
- Agent feedback ratings from `predictor-agent-feedback` pipeline.
- Recency of last update.

Scores feed into the predictor as contextual signals when suggesting
skills to agents during session-start.

---

## Non-Goals

- Hosting a standalone package registry (npm/PyPI style).
- Paid or monetized packages.
- Automatic skill creation from agent behavior (see `adaptive-skill-lifecycle`).
- Runtime sandboxing of marketplace packages (trust model is
  review-gated, not sandboxed).

---

## Integration Contracts

- **Predictor Agent Feedback**: quality ratings from agents feed into
  package scores. The predictor's feedback pipeline provides the
  signal; the marketplace aggregates it per-package.
- **Adaptive Skill Lifecycle**: auto-created skills can be promoted
  to marketplace packages via `signet marketplace publish`.
- **Multi-Agent**: install events scoped to `agent_id` (invariant 1).
  Each agent tracks its own installed packages independently.
- **Skills API**: `POST /api/marketplace/install` calls the existing
  `/api/skills/install` internally for skill-type packages.

---

## Rollout Phases

### Phase 1: Monorepo + Manual Review

Create the marketplace repository. Implement manifest validation
in the reviews worker. `registry.json` generation via GitHub Action.
Daemon caches registry. CLI `search` and `install` commands work.
Review is manual (maintainer approves PRs).

### Phase 2: Automated Review + Feedback

Reviews worker runs full automated checks and posts structured
review comments. Feedback endpoint live. Quality scores computed.
`signet marketplace publish` automates PR creation.

### Phase 3: Predictor Integration + Discovery

Marketplace scores feed into the predictor feature pipeline.
Dashboard surfaces recommended packages based on agent usage
patterns. Update notifications in CLI and dashboard.

---

## Validation and Tests

- Schema validation test: submit manifests with missing fields,
  invalid types, version conflicts — verify rejection messages.
- Install test: install a skill from the marketplace monorepo,
  verify it appears in `$SIGNET_WORKSPACE/skills/` and in
  `/api/skills` response with correct `agent_id`.
- Registry generation test: add/remove package directories, verify
  `registry.json` reflects the correct state after rebuild.
- Feedback test: submit ratings, verify aggregated score updates
  in registry and predictor feature export.

---

## Success Metrics

- A skill published via PR is installable within 5 minutes of merge.
- Registry search returns relevant results for single-word queries
  with <200ms daemon response time.
- Installed marketplace packages survive daemon restart (persisted
  in `$SIGNET_WORKSPACE/.marketplace/installed.json`).

---

## Open Decisions

1. **Monorepo hosting** — `signetai/marketplace` on GitHub is the
   default. Should we support self-hosted forks for enterprise users
   with private marketplace registries?
2. **Version conflict resolution** — when a user has a local skill
   with the same name as a marketplace package, which takes priority?
   Leaning toward local-wins with a warning.
3. **Telemetry opt-in** — download count tracking requires opt-in
   telemetry. What is the minimum viable stats endpoint design that
   preserves privacy?
