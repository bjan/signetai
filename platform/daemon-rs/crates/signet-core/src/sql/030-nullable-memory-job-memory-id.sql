-- Migration 030: Make memory_jobs.memory_id nullable
--
-- The memory_jobs table was created in 002 with `memory_id TEXT NOT NULL`.
-- Migration 007 added document_id for document_ingest jobs, but those jobs
-- don't have a memory_id at creation time -- they produce memories later.
-- The NOT NULL constraint causes POST /api/documents to fail with:
--   SQLiteError: NOT NULL constraint failed: memory_jobs.memory_id
--
-- SQLite doesn't support ALTER COLUMN, so we rebuild the table.
--
-- NOTE: document_id appears in memory_jobs_new below because it was added by
-- migration 007 and must be preserved in the rebuild. This is NOT a duplicate
-- column definition — migration 007 used ALTER TABLE ADD COLUMN (via the Rust
-- add_column_if_missing helper) while this migration replaces the whole table.
-- The SELECT...FROM memory_jobs at the end copies all existing document_id data.

CREATE TABLE IF NOT EXISTS memory_jobs_new (
    id TEXT PRIMARY KEY,
    memory_id TEXT,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    payload TEXT,
    result TEXT,
    attempts INTEGER DEFAULT 0,
    max_attempts INTEGER DEFAULT 3,
    leased_at TEXT,
    completed_at TEXT,
    failed_at TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    document_id TEXT,
    FOREIGN KEY (memory_id) REFERENCES memories(id)
);

INSERT INTO memory_jobs_new
    (id, memory_id, job_type, status, payload, result,
     attempts, max_attempts, leased_at, completed_at, failed_at,
     error, created_at, updated_at, document_id)
SELECT
    id, memory_id, job_type, status, payload, result,
    attempts, max_attempts, leased_at, completed_at, failed_at,
    error, created_at, updated_at, document_id
FROM memory_jobs;

DROP TABLE IF EXISTS memory_jobs;
ALTER TABLE memory_jobs_new RENAME TO memory_jobs;

CREATE INDEX IF NOT EXISTS idx_memory_jobs_status
    ON memory_jobs(status);
CREATE INDEX IF NOT EXISTS idx_memory_jobs_memory_id
    ON memory_jobs(memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_jobs_completed_at
    ON memory_jobs(completed_at);
CREATE INDEX IF NOT EXISTS idx_memory_jobs_failed_at
    ON memory_jobs(failed_at);
