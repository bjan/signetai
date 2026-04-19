-- Migration 003: Unique content hash index
--
-- Safely creates a unique partial index on content_hash. Must run as a
-- separate migration because 002 was already shipped with a non-unique
-- index -- existing installs need this new version to pick up the change.
--
-- Also backfills `why` and `project` columns for databases that ran 001
-- before those columns were added to the baseline CREATE TABLE.

-- Columns added programmatically in migrations.rs:
--   memories.why TEXT
--   memories.project TEXT

-- Drop the non-unique index from migration 002 if it exists
DROP INDEX IF EXISTS idx_memories_content_hash;

-- Deduplicate content_hash values before enforcing uniqueness.
-- For each group of duplicates, keep the semantically newest row
-- (by created_at) and null out the hash on all others.
UPDATE memories
SET content_hash = NULL
WHERE content_hash IS NOT NULL
  AND is_deleted = 0
  AND id NOT IN (
    SELECT id FROM (
        SELECT id, ROW_NUMBER() OVER (
            PARTITION BY content_hash
            ORDER BY created_at DESC, rowid DESC
        ) AS rn
        FROM memories
        WHERE content_hash IS NOT NULL
          AND is_deleted = 0
    ) ranked
    WHERE rn = 1
  );

-- Now safe to create the unique partial index
CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_content_hash_unique
    ON memories(content_hash)
    WHERE content_hash IS NOT NULL AND is_deleted = 0;
