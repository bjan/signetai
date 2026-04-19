-- Migration 010: UMAP Cache
--
-- Adds a table for caching UMAP dimensionality reduction results.

CREATE TABLE IF NOT EXISTS umap_cache (
    id INTEGER PRIMARY KEY,
    dimensions INTEGER NOT NULL,
    embedding_count INTEGER NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL
);
