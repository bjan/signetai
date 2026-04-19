---
title: "Adaptive Skill Lifecycle"
id: adaptive-skill-lifecycle
status: planning
informed_by: []
section: "Procedural Memory"
depends_on:
  - "procedural-memory-plan"
  - "predictor-agent-feedback"
success_criteria:
  - "Skills are continuously maintained and performance-scored from invocation outcomes"
scope_boundary: "Passive skill creation, outcome scoring, and deprecation automation. Does not cover skill marketplace distribution (see git-marketplace-monorepo) or manual skill authoring workflows."
draft_quality: "auto-generated, needs user validation before implementation"
---

# Adaptive Skill Lifecycle

*Skills emerge from repeated behavior, improve from outcomes, and
deprecate when they stop working.*

---

## Problem Statement

Skills in Signet are currently static artifacts. A human writes a
skill file, installs it, and the agent uses it. There is no mechanism
for agents to observe that they perform the same multi-tool sequence
repeatedly and crystallize it into a reusable skill. There is no
feedback loop from invocation outcomes to skill quality. There is no
automatic deprecation when a skill's success rate drops below a
threshold.

Procedural memory (P1 complete) provides the graph node foundation:
`skill_meta` table, skill entities with `entity_type = 'skill'`,
decay rates, and usage tracking via `POST /api/skills/used`. But the
lifecycle — creation from patterns, scoring from outcomes, evolution
from feedback — does not exist yet.

---

## Goals

1. Detect repeated multi-tool sequences in session transcripts and propose skill candidates automatically.
2. Score skill quality from invocation outcomes (success/failure, agent feedback ratings).
3. Automatically deprecate skills whose success rate drops below a configurable threshold.
4. Update skill content when the agent's behavior for the same task pattern evolves.
5. Feed skill lifecycle signals (creation, invocation, deprecation) into the predictive scorer.

---

## Proposed Capability Set

### A. Pattern Detection (Passive Skill Creation)

The pipeline's extraction stage already processes session transcripts.
A new post-extraction analysis pass identifies repeated tool sequences:

1. After extraction, the transcript is scanned for tool-call sequences
   (3+ consecutive tool invocations forming a coherent operation).
2. Sequences are hashed by tool names + parameter shapes (not values).
3. When the same hash appears across 3+ distinct sessions for the same
   `agent_id`, a skill candidate is created.
4. The candidate is stored in a new `skill_candidates` table:

```sql
CREATE TABLE skill_candidates (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id     TEXT NOT NULL DEFAULT 'default',
    hash         TEXT NOT NULL,
    name         TEXT NOT NULL,
    description  TEXT,
    tool_sequence TEXT NOT NULL,  -- JSON array of tool names + param shapes
    session_keys TEXT NOT NULL,   -- JSON array of originating session keys
    occurrences  INTEGER NOT NULL DEFAULT 1,
    status       TEXT NOT NULL DEFAULT 'candidate'
                 CHECK(status IN ('candidate', 'promoted', 'dismissed')),
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    promoted_at  TEXT,
    UNIQUE(agent_id, hash)
);
```

5. Candidates with 3+ occurrences are auto-promoted to real skills:
   a skill file is written to `$SIGNET_WORKSPACE/skills/`, a
   `skill_meta` row is created, and a skill entity is registered in
   the knowledge graph with `entity_type = 'skill'`.

### B. Outcome Scoring

Every skill invocation already records usage via
`POST /api/skills/used` (procedural memory P2). This spec adds
outcome tracking:

- The `skill_meta` table gains `success_count`, `failure_count`, and
  `last_outcome` columns (new migration).
- After a skill invocation, the agent (or the pipeline's session-end
  analysis) records whether the skill achieved its goal. Sources:
  - Explicit agent feedback via `memory_feedback` MCP tool with
    `target_type = 'skill'`.
  - Implicit signal: if the agent retries the same task with different
    tools after a skill invocation, that counts as a failure signal.
  - Session summary analysis: the summary worker checks if skill
    invocations correlated with task completion.
- Success rate = `success_count / (success_count + failure_count)`.
  Exposed via `GET /api/skills/:name/stats`.

### C. Deprecation Automation

Skills with success rate below a configurable threshold (default: 30%)
over a rolling window (default: last 20 invocations) are flagged:

1. **Warning** at 40%: dashboard shows yellow indicator, agent
   receives a note in skill suggestions that the skill has low
   reliability.
2. **Deprecated** at 30%: skill is removed from `suggest` results.
   The skill file remains on disk but `skill_meta.status` is set to
   `deprecated`. Entity attributes gain a `deprecated_at` attribute.
3. **Manual override**: users can pin a skill as `protected` to
   prevent auto-deprecation. Protected status is stored in
   `skill_meta` and respected by the deprecation sweep.

The deprecation sweep runs as part of the pipeline's maintenance
stage (existing maintenance worker in
`platform/daemon/src/pipeline/`).

### D. Skill Evolution

When an agent's behavior for a detected pattern changes (new tools
added, parameter shapes shift), the system updates the skill rather
than creating a duplicate:

- Pattern hash comparison: if a candidate's tool sequence shares 70%+
  overlap with an existing skill's sequence, the existing skill is
  updated rather than a new one created.
- Version history: `skill_meta` gains a `version` column (integer,
  auto-incremented on update). The skill file is overwritten; the
  previous version is recoverable from git history
  (`$SIGNET_WORKSPACE/` is auto-committed).
- Evolution events are logged in `entity_attributes` as temporal
  markers for the skill entity.

### E. Scorer Integration

Skill lifecycle signals become predictor features:

- `skill_age_days`: time since creation.
- `skill_success_rate`: rolling success rate.
- `skill_invocation_velocity`: invocations per day (7-day window).
- `skill_version`: current version number.
- `is_auto_created`: boolean distinguishing passive-created vs
  manually authored skills.

These join the existing `decay_rate`, `use_count`, `last_used_at`
features from procedural memory.

---

## Non-Goals

- Manual skill authoring UI in the dashboard (existing flow is fine).
- Skill marketplace publishing (see `git-marketplace-monorepo`).
- Cross-agent skill sharing or permission models (handled by
  multi-agent skill scoping invariant).
- Natural language skill description generation via LLM (defer to
  future iteration; v1 uses extracted tool names and param shapes).

---

## Integration Contracts

- **Procedural Memory**: this spec builds directly on P1-P2
  (`skill_meta`, `POST /api/skills/used`). Pattern detection runs
  after P2's usage tracking provides the invocation data. P3's
  implicit relation computation benefits from auto-created skills
  expanding the skill graph.
- **Predictor Agent Feedback**: the feedback MCP tool provides
  explicit outcome signals. Without this dependency, outcome scoring
  relies only on implicit signals (retries, session analysis).
- **Knowledge Architecture**: auto-created skills become skill
  entities with aspects and attributes via KA structural assignment
  (existing pipeline stage). Deprecation writes `deprecated_at`
  attributes respecting the entity taxonomy (invariant 3).
- **Multi-Agent**: `skill_candidates` and all `skill_meta` columns
  are scoped by `agent_id` (invariant 1). Each agent develops its
  own skill repertoire independently.
- **Constraints**: if a skill has constraints (e.g., "only use in
  production environments"), they surface unconditionally per
  invariant 5, regardless of skill score.

---

## Rollout Phases

### Phase 1: Pattern Detection + Candidate Table

Ship `skill_candidates` migration. Post-extraction analysis detects
repeated tool sequences and creates candidate rows. Dashboard shows
candidate list. No auto-promotion yet — users manually promote via
`signet skill promote <candidate-id>`.

### Phase 2: Outcome Scoring + Auto-Promotion

`skill_meta` gains outcome columns. Agent feedback and implicit
signals record outcomes. Candidates with 3+ occurrences auto-promote.
Deprecation sweep runs in maintenance stage. Scorer features exported.

### Phase 3: Skill Evolution + Full Automation

Pattern overlap detection updates existing skills instead of creating
duplicates. Version tracking enabled. Dashboard shows skill health
timeline (success rate over time, version history, deprecation risk).

---

## Validation and Tests

- Pattern detection test: feed 3 sessions with identical tool
  sequences to the extraction pipeline, verify a `skill_candidates`
  row is created with `occurrences = 3`.
- Outcome scoring test: record 5 success and 15 failure invocations,
  verify success rate is 25% and deprecation flag triggers.
- Auto-promotion test: verify candidate with 3+ occurrences produces
  a skill file in `$SIGNET_WORKSPACE/skills/` and a `skill_meta` row.
- Evolution test: modify a pattern's tool sequence with 80% overlap,
  verify the existing skill is updated (version incremented) rather
  than a new skill created.
- Agent scoping test: verify candidates from agent A are not visible
  to agent B in isolated mode.

---

## Success Metrics

- At least 1 skill candidate is surfaced per 10 active sessions
  where the agent performs repetitive multi-tool operations.
- Auto-created skills achieve >50% success rate within their first
  20 invocations (quality bar for auto-promotion).
- Skills with <30% success rate are deprecated within 24 hours of
  crossing the threshold.

---

## Open Decisions

1. **Promotion threshold** — 3 occurrences across 3 sessions is the
   proposed bar. Should this be configurable in `agent.yaml`? Higher
   thresholds reduce noise but delay useful skill creation.
2. **LLM-assisted descriptions** — v1 uses mechanical tool-sequence
   descriptions. Should Phase 2 add an optional LLM pass to generate
   natural-language skill descriptions from the tool sequence?
3. **Cross-session pattern window** — should pattern detection look
   at all-time history or a rolling window (e.g., last 30 days)?
   All-time catches rare but valuable patterns; rolling avoids
   promoting obsolete sequences.
