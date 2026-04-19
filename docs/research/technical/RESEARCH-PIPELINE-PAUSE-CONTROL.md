---
title: "Pipeline Pause Control Research"
question: "How should Signet let an operator temporarily stop extraction work to free local resources without dropping queued memory capture?"
---

# Pipeline Pause Control Research

## Question

How should Signet let an operator temporarily stop extraction work to free
local resources without dropping queued memory capture?

## Current behavior

Signet already has several pipeline-adjacent switches, but none match the
operator need cleanly:

1. `memory.pipelineV2.enabled = false`
   - fully disables pipeline enqueue/start behavior
   - reduces work, but new memories stop entering the extraction queue
   - not a good fit for "pause now, catch up later"

2. `memory.pipelineV2.shadowMode = true`
   - still runs extraction and decision stages
   - preserves observability, but does not reduce model/runtime pressure

3. `memory.pipelineV2.mutationsFrozen = true`
   - blocks writes and destructive actions
   - extraction still runs, so local compute and provider pressure remain

The missing mode is operational pause:

- keep raw memory capture working
- allow pending jobs to accumulate safely
- stop worker activity so the machine gets relief
- resume later and drain backlog with existing pipeline behavior

## Constraints from the current codebase

### Daemon startup

`platform/daemon/src/daemon.ts` decides whether to start the extraction
pipeline at startup. If the pipeline does not start, Signet already falls
back to retention-only background work.

### Enqueue behavior

Remember/session-end paths check `pipelineV2.enabled` before enqueueing
jobs. That means a pause mechanism must remain distinct from `enabled`
if we want backlog preservation.

### Worker lifecycle

`platform/daemon/src/pipeline/index.ts` already exposes a clean
`startPipeline()` / `stopPipeline()` split. This is a strong fit for a
paused state because the worker graph is already centralized.

## Recommended design

Add a dedicated persisted flag:

```yaml
memory:
  pipelineV2:
    paused: true
```

Semantics:

1. `enabled = true`, `paused = false`
   - normal operation

2. `enabled = true`, `paused = true`
   - keep enqueue behavior
   - do not start extraction workers
   - surface pipeline mode as `paused`

3. `enabled = false`
   - fully disabled, regardless of paused flag

This gives users a real "pause/resume" control instead of overloading
`enabled` or `mutationsFrozen`.

## Recommendation for CLI behavior

Expose operator-first commands:

- `signet daemon pause`
- `signet daemon resume`

The CLI should:

1. persist the `paused` flag in the active config file
2. restart the daemon so the running process picks up the new state
3. report whether queued work will resume later

Restart-on-toggle is acceptable for the first implementation because the
current daemon does not hot-reload pipeline runtime state from `agent.yaml`.

## Non-goals

- no scheduler or timed auto-resume
- no per-worker selective pause
- no change to memory retention semantics
- no dropping or reclassifying existing queued jobs
