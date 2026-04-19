-- Migration 018: Skill Meta
--
-- Creates the skill_meta table for procedural memory P1.
-- Skills become first-class nodes in the knowledge graph,
-- with per-agent scoping, usage tracking, and decay.
--
-- NOTE: Uses CREATE TABLE (not IF NOT EXISTS) because the TS
-- migration guards with an existence check. The Rust runner
-- should also guard or use IF NOT EXISTS.

CREATE TABLE IF NOT EXISTS skill_meta (
    entity_id     TEXT PRIMARY KEY REFERENCES entities(id),
    agent_id      TEXT NOT NULL DEFAULT 'default',
    version       TEXT,
    author        TEXT,
    license       TEXT,
    source        TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'utility',
    triggers      TEXT,
    tags          TEXT,
    permissions   TEXT,
    enriched      INTEGER DEFAULT 0,
    installed_at  TEXT NOT NULL,
    last_used_at  TEXT,
    use_count     INTEGER DEFAULT 0,
    importance    REAL DEFAULT 0.7,
    decay_rate    REAL DEFAULT 0.99,
    fs_path       TEXT NOT NULL,
    uninstalled_at TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_skill_meta_agent ON skill_meta(agent_id);
CREATE INDEX IF NOT EXISTS idx_skill_meta_source ON skill_meta(source);
