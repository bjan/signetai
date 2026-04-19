-- Migration 017: Task Skills
--
-- Adds optional skill attachment to scheduled tasks.
-- Tasks can reference a skill by name and specify a mode
-- for how the skill content is integrated into the prompt.

-- Columns added programmatically in migrations.rs:
--   scheduled_tasks.skill_name TEXT
--   scheduled_tasks.skill_mode TEXT CHECK (skill_mode IN ('inject', 'slash') OR skill_mode IS NULL)
