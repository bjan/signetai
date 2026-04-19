-- Migration 023: Predictor Columns on session_memories
--
-- Adds predictor_rank column to session_memories for Sprint 2 scoring
-- integration. The predictor_score and final_score columns already exist
-- from migration 015; entity_slot, aspect_slot, is_constraint, and
-- structural_density were added in migration 020.

-- Columns added programmatically in migrations.rs:
--   session_memories.predictor_rank INTEGER
