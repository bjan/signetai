-- Migration 004: Actor classification + correlation + retention indexes
--
-- Adds structured actor_type and request/session correlation fields to
-- memory_history, plus indexes to support the retention worker's purge
-- queries on soft-deleted memories, expired history, and completed jobs.

-- Columns added programmatically in migrations.rs:
--   memory_history.actor_type TEXT
--   memory_history.session_id TEXT
--   memory_history.request_id TEXT

-- Retention worker indexes

-- Tombstone purge: find soft-deleted memories past retention window
CREATE INDEX IF NOT EXISTS idx_memories_deleted_at
    ON memories(deleted_at)
    WHERE is_deleted = 1;

-- History purge: find old history events by date
CREATE INDEX IF NOT EXISTS idx_memory_history_created_at
    ON memory_history(created_at);

-- Job purge: find completed/dead jobs by date
CREATE INDEX IF NOT EXISTS idx_memory_jobs_completed_at
    ON memory_jobs(completed_at)
    WHERE status = 'completed';

CREATE INDEX IF NOT EXISTS idx_memory_jobs_failed_at
    ON memory_jobs(failed_at)
    WHERE status = 'dead';
