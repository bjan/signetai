---
title: "Code Ownership and Review SLA"
id: code-ownership-sla
status: planning
informed_by: []
section: "Process"
depends_on:
  - "multi-agent-support"
success_criteria:
  - "CODEOWNERS and reviewer ownership map reduce review-routing latency"
scope_boundary: "GitHub CODEOWNERS file, a review SLA tracking script, and process documentation. No application code changes."
draft_quality: "auto-generated, needs user validation before implementation"
---

# Code Ownership and Review SLA

## Problem

Review requests are not systematically routed. PRs sit without review
because no one is explicitly responsible for a package area. When
reviews do happen, there are no defined SLAs, so priority bugs
compete with feature work for reviewer attention. Agent-driven
reviews in CI partially help but lack ownership context.

## Goals

1. Every package directory has an explicit owner (human or agent).
2. PRs are auto-assigned to the correct reviewer via CODEOWNERS.
3. Review SLAs are defined by severity and tracked.

## Non-goals

- Enforcing SLAs with automated escalation (tracking only for now).
- Changing who can merge (ownership is about review, not merge rights).

## Proposed approach

**CODEOWNERS file** (`.github/CODEOWNERS`): Map each package
directory and key shared files to their owner:

```
platform/core/          @nicholai
platform/daemon/        @nicholai
surfaces/cli/           @nicholai
libs/sdk/               @nicholai
integrations/*/connector/ @nicholai
web/                    @nicholai
docs/                   @nicholai
```

As the team grows, owners diversify. Agent reviewers are listed as
secondary reviewers where supported by the CI agent review system.

**Review SLA tiers**:

- **P0 (production incident)**: First review within 4 hours.
- **P1 (bug fix, blocking issue)**: First review within 24 hours.
- **P2 (feature, improvement)**: First review within 1 week.

Priority is determined by PR labels (`priority:p0`, `priority:p1`,
`priority:p2`). Default is P2 if no label is set.

**SLA tracking script** (`scripts/review-sla.ts`): Runs on a
schedule (daily). Uses `gh pr list` and `gh api` to check open PRs,
compute time since review was requested, and flag PRs exceeding their
SLA. Posts a summary to a configured channel (GitHub issue, Discord,
or stdout).

**PR assignment**: GitHub auto-assigns reviewers from CODEOWNERS on
PR creation. The PR author can override or add reviewers.

## Phases

### Phase 1 -- CODEOWNERS and labeling

- Create `.github/CODEOWNERS` with current ownership map.
- Add priority labels to the GitHub repository.
- Document SLA tiers in CONTRIBUTING.md.

### Phase 2 -- SLA tracking

- Implement `scripts/review-sla.ts` for SLA computation.
- Add scheduled CI job to run SLA check daily.
- Add SLA status to the weekly project summary.

## Validation criteria

- PR touching `platform/daemon/` auto-assigns the daemon owner.
- PR labeled `priority:p0` open for 5 hours triggers an SLA warning.
- CODEOWNERS covers all package directories in the monorepo.

## Open decisions

1. Should SLA violations block merge, or only produce warnings and
   tracking data?
