-- Migration 025: Agent Relevance Feedback
--
-- Adds columns to session_memories for accumulating per-prompt agent
-- feedback scores. The running mean of feedback across the session
-- becomes the primary training label for the predictive scorer.

-- Columns added programmatically in migrations.rs:
--   session_memories.agent_relevance_score REAL
--   session_memories.agent_feedback_count INTEGER DEFAULT 0
