---
title: "Unified Constellation/Embedding/Entity Viewer"
id: constellation-unified-viewer
status: planning
informed_by: []
section: "Dashboard"
depends_on:
  - "knowledge-architecture-schema"
success_criteria:
  - "Realtime unified viewer replaces slow 3D renderer and visualizes new dependency types by default"
scope_boundary: "Covers the visualization component and its daemon data requirements; does not define new knowledge graph schema or change entity taxonomy"
draft_quality: "auto-generated, needs user validation before implementation"
---

# Unified Constellation/Embedding/Entity Viewer

Spec metadata:
- ID: `constellation-unified-viewer`
- Status: `planning`
- Hard depends on: `knowledge-architecture-schema`
- Registry: `docs/specs/INDEX.md`

Related docs:
- `surfaces/dashboard/src/lib/components/embeddings/` (current renderers)
- `docs/DASHBOARD.md` (design tokens, component conventions)
- `docs/specs/complete/knowledge-architecture-schema.md`

---

## 1) Problem

The dashboard has three separate views (embeddings scatter, knowledge entity
list, timeline) with no unified visualization. The previous 3d-force-graph
renderer was abandoned (edge hairball, no hierarchy at scale). Users must
switch tabs to correlate entities with embeddings and temporal patterns.

## 2) Goals

1. Replace separate views with a single force-directed graph with progressive disclosure.
2. Entity nodes at overview; edges on hover/selection only.
3. Click-to-expand for aspects and attributes.
4. Smooth interaction at 1000+ entities via WebGL2.
5. shadcn-svelte for all overlay UI.

## 3) Proposed capability set

### A) WebGL2 canvas renderer

Single Svelte 5 component at `constellation/ConstellationCanvas.svelte`. SDF
shapes per entity type (circle=person, diamond=project, hexagon=system,
square=tool, triangle=concept, star=skill, octagon=task, dot=unknown). Node
size from `mention_count` (clamped 8-40px). Color from `--sig-entity-*` tokens.
Additive glow on hover/selection.

### B) Force-directed layout

`d3-force` with `forceLink` (strength from dependency weight), `forceManyBody`,
`forceCollide`, `forceCenter`. Positions cached in state; simulation only on
load and filter changes.

### C) Progressive disclosure

**Overview**: nodes only, labels for `mention_count >= 5`, zoom/pan.
**Focus**: hover reveals connected edges. Tooltip: name, type, count, top 3 aspects.
**Expand**: click loads aspects (satellite nodes) and attributes (shadcn Sheet
panel). Constraint attributes visually distinguished.

### D) Data endpoints

Consumes existing daemon API — no new endpoints:
- `GET /api/knowledge/entities` — list with type, count.
- `GET /api/knowledge/entities/:id/dependencies` — edges on demand.
- `GET /api/knowledge/entities/:id/aspects` — aspects/attributes on demand.

All queries agent-scoped via daemon session context.

### E) Filter overlay

shadcn Popover: entity type checkboxes, mention count threshold slider, text
search (highlights matches, dims others), dependency type filter. `/` focuses
search, `Escape` clears.

### F) Realtime updates

Subscribe to daemon SSE at `/api/events`. Add/reposition/remove nodes on
entity change events. Debounce at 500ms.

## 4) Non-goals

- No 3D rendering (2D WebGL2 only).
- No UMAP/t-SNE scatter (separate concern).
- No schema changes or new entity types.
- No editing — read-only viewer.

## 5) Integration contracts

**Viewer <-> Knowledge Architecture**: Consumes `memory_entities`,
`entity_dependencies`, `entity_aspects`, `entity_attributes`. Entity types
match taxonomy enum. Dependency `strength`/`type` drive edge rendering.

**Viewer <-> Dashboard IA**: First-class tab, replaces `embeddings` and
`knowledge` tabs. Tab ID: `constellation`.

**Viewer <-> Scorer**: Mention counts and dependency strengths (scorer
features) reflected in node size and edge weight.

## 6) Rollout phases

### Phase 1: Static entity graph
WebGL2 canvas, d3-force layout, type-based shapes, filter panel. Replace
embeddings tab.

### Phase 2: Progressive disclosure
Hover-edges, click-expand, Sheet detail panel, SSE live updates.

### Phase 3: Performance polish
Frustum culling, LOD labels, keyboard nav, animated transitions.

## 7) Validation and tests

- Canvas initializes and draws nodes for mock entity set.
- d3-force produces non-overlapping positions for 100 entities.
- Hover reveals edges, click opens detail panel.
- Type toggles and search correctly show/hide nodes.
- Viewer only displays entities for the active agent.

## 8) Success metrics

- Full graph loads and renders within 500ms.
- Hover-to-edge responds within 16ms (single frame).
- Entity detail loads within 200ms of click.
- Three separate views replaced by one cohesive visualization.

## 9) Open decisions

1. Keep old embeddings scatter as secondary mode or remove entirely.
2. Edge strategy for 50+ dependency entities: cap or bundle.
3. Whether constellation is default landing view or secondary to home.
