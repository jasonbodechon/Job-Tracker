ALTER TABLE jobs ADD COLUMN IF NOT EXISTS scoring_version text NOT NULL DEFAULT '1.0';
CREATE INDEX IF NOT EXISTS jobs_scoring_version_idx ON jobs(user_id, scoring_version) WHERE active=true;
UPDATE profiles
SET weights = '{"title":20,"requiredSkills":30,"preferredSkills":10,"resumeEvidence":20,"experience":15,"education":5}'::jsonb,
    updated_at = now()
WHERE weights ? 'remote' OR weights ? 'freshness';
