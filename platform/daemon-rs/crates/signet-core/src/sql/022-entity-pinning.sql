-- Migration 022: Entity Pinning
--
-- Adds pinning support to entities for user-prioritized graph nodes.

-- Columns added programmatically in migrations.rs:
--   entities.pinned INTEGER NOT NULL DEFAULT 0
--   entities.pinned_at TEXT

CREATE INDEX IF NOT EXISTS idx_entities_pinned ON entities(agent_id, pinned, pinned_at DESC);
