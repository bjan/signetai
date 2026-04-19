-- Benchmark dataset: 1000 memories, 50 entities, 10 sessions, 500 embeddings
-- Used by bench-spec.md for reproducible performance testing.
-- Apply to a fresh DB after migrations: sqlite3 bench.db < bench-dataset.sql
-- NOTE: vec_embeddings population (for sqlite-vec) requires the sqlite-vec
--       extension to be loaded at apply time. The embeddings metadata table
--       is always populated; vec_embeddings must be seeded separately if needed.

-- Insert 1000 memories with varied types
WITH RECURSIVE cnt(x) AS (
  SELECT 1
  UNION ALL
  SELECT x+1 FROM cnt WHERE x < 1000
)
INSERT OR IGNORE INTO memories (id, content, normalized_content, content_hash, type, tags, importance, who, context, created_at, updated_at, is_deleted)
SELECT
  printf('mem-%04d', x),
  'Memory content for benchmark test number ' || x || '. This contains enough text to exercise the FTS5 indexer and produce meaningful search results across different memory types and importance levels.',
  lower('memory content for benchmark test number ' || x || ' this contains enough text to exercise the fts5 indexer'),
  printf('hash%04d', x),
  CASE (x % 4)
    WHEN 0 THEN 'episodic'
    WHEN 1 THEN 'fact'
    WHEN 2 THEN 'preference'
    WHEN 3 THEN 'decision'
  END,
  CASE (x % 3)
    WHEN 0 THEN 'testing,benchmark'
    WHEN 1 THEN 'development,code'
    WHEN 2 THEN 'architecture,design'
  END,
  0.3 + (x % 7) * 0.1,
  CASE (x % 5)
    WHEN 0 THEN 'user'
    WHEN 1 THEN 'system'
    WHEN 2 THEN 'agent'
    WHEN 3 THEN 'user'
    WHEN 4 THEN NULL
  END,
  'benchmark-context',
  datetime('2026-01-01', '+' || x || ' minutes'),
  datetime('2026-01-01', '+' || x || ' minutes'),
  0
FROM cnt;

-- Populate FTS index
INSERT OR IGNORE INTO memories_fts (rowid, content)
SELECT rowid, content FROM memories WHERE is_deleted = 0;

-- Insert 50 entities (schema columns: entity_type, mentions, agent_id)
WITH RECURSIVE cnt(x) AS (
  SELECT 1
  UNION ALL
  SELECT x+1 FROM cnt WHERE x < 50
)
INSERT OR IGNORE INTO entities (id, name, canonical_name, entity_type, mentions, agent_id, created_at, updated_at)
SELECT
  printf('ent-%04d', x),
  'Entity ' || x,
  lower('entity ' || x),
  CASE (x % 5)
    WHEN 0 THEN 'person'
    WHEN 1 THEN 'project'
    WHEN 2 THEN 'technology'
    WHEN 3 THEN 'concept'
    WHEN 4 THEN 'organization'
  END,
  x * 3,
  'bench-agent',
  datetime('2026-01-01'),
  datetime('2026-01-01')
FROM cnt;

-- Insert 10 sessions
WITH RECURSIVE cnt(x) AS (
  SELECT 1
  UNION ALL
  SELECT x+1 FROM cnt WHERE x < 10
)
INSERT OR IGNORE INTO sessions (session_key, harness, started_at, last_activity_at, status, runtime_path)
SELECT
  printf('bench-session-%02d', x),
  CASE (x % 2) WHEN 0 THEN 'claude-code' ELSE 'opencode' END,
  datetime('2026-01-01', '+' || (x * 60) || ' minutes'),
  datetime('2026-01-01', '+' || (x * 60 + 30) || ' minutes'),
  'ended',
  CASE (x % 2) WHEN 0 THEN 'plugin' ELSE 'legacy' END
FROM cnt;

-- Insert 20 pipeline jobs (table: memory_jobs, type column: job_type)
WITH RECURSIVE cnt(x) AS (
  SELECT 1
  UNION ALL
  SELECT x+1 FROM cnt WHERE x < 20
)
INSERT OR IGNORE INTO memory_jobs (id, job_type, memory_id, status, attempts, created_at, updated_at)
SELECT
  printf('job-%04d', x),
  CASE (x % 3)
    WHEN 0 THEN 'extraction'
    WHEN 1 THEN 'summary'
    WHEN 2 THEN 'structural-classify'
  END,
  printf('mem-%04d', x),
  CASE (x % 4)
    WHEN 0 THEN 'pending'
    WHEN 1 THEN 'leased'
    WHEN 2 THEN 'completed'
    WHEN 3 THEN 'dead'
  END,
  CASE WHEN x % 4 = 3 THEN 3 ELSE 0 END,
  datetime('2026-01-01', '+' || x || ' minutes'),
  datetime('2026-01-01', '+' || x || ' minutes')
FROM cnt;

-- Insert 500 embedding metadata rows (embeddings table — always available).
-- Vectors are zeroblob(3072) = 768 float32 dimensions, all zeros.
-- For hybrid-search benchmarks that exercise sqlite-vec, also seed vec_embeddings
-- after loading the extension: INSERT INTO vec_embeddings (id, embedding) SELECT ...
WITH RECURSIVE cnt(x) AS (
  SELECT 1
  UNION ALL
  SELECT x+1 FROM cnt WHERE x < 500
)
INSERT OR IGNORE INTO embeddings (id, content_hash, vector, dimensions, source_type, source_id, chunk_text, created_at)
SELECT
  printf('emb-%04d', x),
  printf('hash%04d', x),
  zeroblob(3072),
  768,
  'memory',
  printf('mem-%04d', x),
  'Memory content for benchmark test number ' || x || '.',
  datetime('2026-01-01', '+' || x || ' minutes')
FROM cnt;
