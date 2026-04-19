-- Migration 008: Unique content_hash on embeddings
--
-- The baseline schema declares content_hash as UNIQUE but older databases
-- may have been created before that constraint was enforced. The pipeline
-- Phase C write path uses ON CONFLICT(content_hash), which requires a
-- unique index. Dedup any collisions then create the index.

-- Keep newest embedding per content_hash, delete older dupes
DELETE FROM embeddings
WHERE rowid NOT IN (
    SELECT MIN(rowid) FROM embeddings
    GROUP BY content_hash
);

-- Drop the non-unique index if it exists
DROP INDEX IF EXISTS idx_embeddings_hash;

-- Create unique index
CREATE UNIQUE INDEX IF NOT EXISTS idx_embeddings_content_hash_unique
    ON embeddings(content_hash);
