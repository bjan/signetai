-- Migration 006: Idempotency key
--
-- Adds idempotency_key column for dedup across runtime paths
-- and runtime_path column to track which path produced each memory.

-- Columns added programmatically in migrations.rs:
--   memories.idempotency_key TEXT
--   memories.runtime_path TEXT

-- Unique partial index created in migrations.rs after column add.
