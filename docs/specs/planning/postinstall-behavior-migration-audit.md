---
title: "Post-Install Behavior Migration Audit"
id: postinstall-behavior-migration-audit
status: planning
informed_by: []
section: "CLI/Daemon"
depends_on:
  - "memory-pipeline-v2"
success_criteria:
  - "All critical post-install behaviors are owned by CLI/daemon flows and not fragile post-install scripts"
scope_boundary: "Audits and migrates existing postinstall behaviors to durable owners; does not add new install-time features"
draft_quality: "auto-generated, needs user validation before implementation"
---

# Post-Install Behavior Migration Audit

Spec metadata:
- ID: `postinstall-behavior-migration-audit`
- Status: `planning`
- Hard depends on: `memory-pipeline-v2`
- Registry: `docs/specs/INDEX.md`

Related docs:
- `surfaces/cli/src/cli.ts` (CLI entry point)
- `platform/daemon/src/daemon.ts` (daemon startup)
- `platform/core/src/migrations/` (database migrations)

---

## 1) Problem

npm/bun `postinstall` scripts can be skipped (`--ignore-scripts`), disabled by
security policies, or fail silently in CI. Any critical behavior dependent on
postinstall — database migration, config scaffolding, identity file creation,
connector hook installation — is fragile. If a user installs with
`--ignore-scripts` and starts the daemon, the system must still work.

## 2) Goals

1. Catalog every postinstall-triggered behavior across all packages.
2. Verify each critical behavior has a durable owner (CLI setup or daemon startup).
3. Migrate any behaviors lacking a durable owner.
4. Ensure clean boot from `--ignore-scripts` install.
5. Add CI cold-start regression test.

## 3) Proposed capability set

### A) Audit inventory

Check every `package.json` for `postinstall`, `prepare`, and `install` scripts.
Document: package, script content, what it does, criticality tier.

### B) Behavior classification

**Tier 1 — Must run before daemon starts**: database migrations, `~/.agents/`
scaffolding, SQLite creation and WAL setup.

**Tier 2 — Must run before first session**: connector hook installation, MCP
server registration, skills directory and symlinks.

**Tier 3 — Cosmetic / dev-only**: type generation, dashboard build, dev tools.

### C) Durable owner assignment

| Behavior | Owner | When |
|---|---|---|
| Database migrations | Daemon startup | Before HTTP bind |
| Directory scaffolding | CLI setup + daemon startup | Both create `~/.agents/` if missing |
| Config file creation | CLI setup | `agent.yaml` with defaults |
| Identity files | CLI setup | Template AGENTS.md, SOUL.md, etc. |
| Connector hooks | CLI setup | Per-harness `install()` |
| Skills directory | CLI setup + daemon startup | Create if missing |

Daemon startup is self-healing: missing `~/.agents/` or `agent.yaml` are
created with safe defaults. Missing optional files (SOUL.md, IDENTITY.md)
logged but not fatal.

### D) Daemon cold-start guard

Startup validation in `daemon.ts` before HTTP bind:
1. Check/create `~/.agents/` with minimal scaffolding.
2. Check/create `agent.yaml` with defaults.
3. Run migrations unconditionally (already happens — verify no conditionality).
4. Verify `memories.db` accessible and WAL mode set.
5. Log results, never crash on missing optional files.

### E) Postinstall cleanup

Remaining scripts must be Tier 3 only, guarded with `|| true`, and commented
to document durable fallback exists. Remove any that duplicate CLI/daemon-
owned behavior.

### F) CI cold-start test

CI job that: installs with `--ignore-scripts`, starts daemon, verifies
`GET /health` returns 200, sends session-start hook, checks `~/.agents/`
was auto-scaffolded.

## 4) Non-goals

- No changes to install UX or setup wizard flow.
- No new CLI commands.
- No changes to the migration system itself.
- No package manager migration.

## 5) Integration contracts

**Audit <-> Daemon Startup**: Daemon reaches healthy state given only the
installed package and `SIGNET_PATH`. No assumptions about prior CLI runs.

**Audit <-> CLI Setup**: `signet setup` remains recommended first-run.
Additive, not destructive — re-running preserves user data.

**Audit <-> Migrations**: Run unconditionally at daemon startup, idempotent.

**Audit <-> Connectors**: `install()` only called from CLI setup. Daemon
works without hooks (no lifecycle, but no crash).

## 6) Rollout phases

### Phase 1: Audit and document
Inventory all packages, classify behaviors, identify gaps.

### Phase 2: Migrate and guard
Move behaviors to durable owners, add cold-start guard, add CI test,
clean up postinstall scripts.

## 7) Validation and tests

- Cold-start: `--ignore-scripts` install, daemon starts, health OK.
- Idempotent setup: `signet setup` twice, no corruption.
- Migration: daemon starts with empty `~/.agents/`, creates schema.
- Scaffolding: daemon creates `agent.yaml` with valid defaults.
- Remaining postinstall scripts are Tier 3 and guarded.

## 8) Success metrics

- Daemon reaches healthy state from `--ignore-scripts` install within 5s.
- Zero Tier 1/2 behaviors depend solely on postinstall scripts.
- CI cold-start test passes on every PR.

## 9) Open decisions

1. Whether daemon cold-start guard runs full non-interactive setup or only
   scaffolds minimal required files.
2. Whether to bundle this with a `signet doctor` command (see
   developer-doctor-command spec) or keep separate.
3. Whether to remove all postinstall scripts or keep Tier 3 for dev
   convenience.
