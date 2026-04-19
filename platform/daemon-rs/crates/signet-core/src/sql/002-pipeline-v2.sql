-- Migration 002: Pipeline v2 schema
--
-- Adds columns for the memory pipeline (content hashing, soft delete,
-- extraction tracking), plus new tables for history, jobs, and the
-- entity graph.

-- Columns added programmatically in migrations.rs:
--   memories.content_hash TEXT
--   memories.normalized_content TEXT
--   memories.is_deleted INTEGER DEFAULT 0
--   memories.deleted_at TEXT
--   memories.extraction_status TEXT DEFAULT 'none'
--   memories.embedding_model TEXT
--   memories.extraction_model TEXT
--   memories.update_count INTEGER DEFAULT 0
--   memories.who TEXT
--   memories.why TEXT
--   memories.project TEXT
--   memories.pinned INTEGER DEFAULT 0
--   memories.importance REAL DEFAULT 0.5
--   memories.last_accessed TEXT
--   memories.access_count INTEGER DEFAULT 0

-- memory_history (immutable audit trail)
CREATE TABLE IF NOT EXISTS memory_history (
    id TEXT PRIMARY KEY,
    memory_id TEXT NOT NULL,
    event TEXT NOT NULL,
    old_content TEXT,
    new_content TEXT,
    changed_by TEXT NOT NULL,
    reason TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (memory_id) REFERENCES memories(id)
);

-- memory_jobs (durable queue)
CREATE TABLE IF NOT EXISTS memory_jobs (
    id TEXT PRIMARY KEY,
    memory_id TEXT NOT NULL,
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
    FOREIGN KEY (memory_id) REFERENCES memories(id)
);

-- Entity graph
CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    entity_type TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS relations (
    id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    strength REAL DEFAULT 1.0,
    metadata TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (source_entity_id) REFERENCES entities(id),
    FOREIGN KEY (target_entity_id) REFERENCES entities(id)
);

CREATE TABLE IF NOT EXISTS memory_entity_mentions (
    memory_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    PRIMARY KEY (memory_id, entity_id),
    FOREIGN KEY (memory_id) REFERENCES memories(id),
    FOREIGN KEY (entity_id) REFERENCES entities(id)
);

-- Audit table for migration history
CREATE TABLE IF NOT EXISTS schema_migrations_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version INTEGER NOT NULL,
    applied_at TEXT NOT NULL,
    duration_ms INTEGER,
    checksum TEXT
);

-- Indexes on programmatically-added columns (content_hash, is_deleted,
-- extraction_status) are created in migrations.rs after column adds.

CREATE INDEX IF NOT EXISTS idx_memory_history_memory_id
    ON memory_history(memory_id);
CREATE INDEX IF NOT EXISTS idx_memory_jobs_status
    ON memory_jobs(status);
CREATE INDEX IF NOT EXISTS idx_memory_jobs_memory_id
    ON memory_jobs(memory_id);
CREATE INDEX IF NOT EXISTS idx_relations_source
    ON relations(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_relations_target
    ON relations(target_entity_id);
CREATE INDEX IF NOT EXISTS idx_memory_entity_mentions_entity
    ON memory_entity_mentions(entity_id);
