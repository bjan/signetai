-- Migration 005: Extended graph columns
--
-- Extends entities, relations, and memory_entity_mentions with
-- additional columns for canonical names, mention counts, embeddings,
-- confidence, and temporal data.

-- Columns added programmatically in migrations.rs:
--   entities.canonical_name TEXT
--   entities.mentions INTEGER DEFAULT 0
--   entities.embedding BLOB
--   relations.mentions INTEGER DEFAULT 1
--   relations.confidence REAL DEFAULT 0.5
--   relations.updated_at TEXT
--   memory_entity_mentions.mention_text TEXT
--   memory_entity_mentions.confidence REAL
--   memory_entity_mentions.created_at TEXT

-- Index on canonical_name created in migrations.rs after column add.

-- Composite index for traversing outgoing edges by type
CREATE INDEX IF NOT EXISTS idx_relations_composite ON relations(source_entity_id, relation_type);
