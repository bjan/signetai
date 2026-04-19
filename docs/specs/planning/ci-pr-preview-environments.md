---
title: "PR Preview Environments and UI Smoke Tests"
id: ci-pr-preview-environments
status: planning
informed_by: []
section: "DevEx"
depends_on:
  - "signet-runtime"
success_criteria:
  - "Each frontend-affecting PR can publish a preview with automated smoke and screenshot checks"
scope_boundary: "GitHub Actions workflows and Cloudflare Pages configuration. No changes to application source beyond build config for preview URLs."
draft_quality: "auto-generated, needs user validation before implementation"
---

# PR Preview Environments and UI Smoke Tests

## Problem

Frontend changes to the marketing site (web/) and dashboard
(surfaces/dashboard/) cannot be visually reviewed without
checking out the branch locally and running a dev server. This slows
review for UI PRs and makes regressions in layout or styling easy to
miss.

## Goals

1. PRs touching web/ deploy a preview to Cloudflare Pages with a
   stable URL posted as a PR comment.
2. PRs touching dashboard/ deploy a preview build accessible for
   visual review.
3. Optional smoke tests validate that key pages render without errors.

## Non-goals

- Full end-to-end testing with a live daemon backend.
- Preview environments for API-only changes.
- Persisting preview deployments beyond PR lifetime.

## Proposed approach

**Web previews (Cloudflare Pages)**: Cloudflare Pages natively
supports preview deployments on non-production branches. Configure
the Pages project to auto-deploy preview builds for PRs. Wrangler
is already used for production deploys (`bun run deploy:web`). The
CI workflow calls `wrangler pages deploy` with the PR branch, and
Cloudflare assigns a `<hash>.signetai-sh.pages.dev` URL.

**Dashboard previews**: Build the dashboard static output
(`surfaces/dashboard/build/`) in CI. Upload the build artifact
to Cloudflare Pages as a separate project (or a subdirectory deploy).
Since the dashboard calls daemon APIs at localhost:3850, the preview
is visual-only (no live data). Add a banner component that renders
when `PREVIEW=true` indicating the backend is unavailable.

**PR comment**: A CI step uses `gh pr comment` to post the preview
URL(s) on the PR. Update the comment on subsequent pushes rather
than creating new comments (use a marker string to find/replace).

**Smoke tests**: After the preview is deployed, run a lightweight
check: fetch the preview URL, verify 200 status, check that the HTML
contains expected landmarks (title, nav, main content area). Use
`curl` or a small script, not a full browser framework.

## Phases

### Phase 1 -- Web preview deploys

- Configure Cloudflare Pages for preview branch deploys.
- Add CI workflow step to trigger deploy and capture URL.
- Post preview URL as PR comment via `gh pr comment`.
- Run basic HTTP smoke test (200 status, title present).

### Phase 2 -- Dashboard preview and cleanup

- Add dashboard static build upload to a preview Pages project.
- Add PREVIEW mode banner to dashboard for non-functional backends.
- Configure automatic preview cleanup when PR is closed/merged.
- Add screenshot comparison step (optional, stretch goal).

## Validation criteria

- PR touching `web/marketing/src/` gets a preview URL posted within 3 minutes.
- PR touching `surfaces/dashboard/` gets a dashboard preview URL.
- Preview URLs return 200 and render the expected page structure.
- Closed PRs have their preview deployments cleaned up.

## Open decisions

1. Should dashboard previews use a mock API layer for interactive
   testing, or is visual-only sufficient for review?
2. Should preview deploys be gated behind a label (e.g.,
   `preview-deploy`) to avoid burning Cloudflare build minutes on
   every PR?
