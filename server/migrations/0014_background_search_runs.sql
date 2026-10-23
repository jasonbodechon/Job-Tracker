-- Searches now run in the background and report their state and progress.
ALTER TABLE search_runs ADD COLUMN status text NOT NULL DEFAULT 'Completed'
  CHECK (status IN ('Running', 'Completed', 'Failed', 'Abandoned'));
ALTER TABLE search_runs ADD COLUMN progress text NOT NULL DEFAULT '';
ALTER TABLE search_runs ADD COLUMN result jsonb;
ALTER TABLE search_runs ADD COLUMN heartbeat_at timestamptz;

-- Earlier versions ran each search inside one web request, so any search that never
-- finished was stopped when that request closed. Record those as abandoned.
UPDATE search_runs
SET status = 'Abandoned',
    finished_at = to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"+00:00"'),
    progress = 'Stopped before finishing',
    errors = errors || '["The search stopped before finishing. Run it again."]'::jsonb
WHERE finished_at IS NULL;

-- At most one running search per user, enforced by the database so double clicks
-- and overlapping scheduled runs cannot start a second one.
CREATE UNIQUE INDEX search_runs_one_running_per_user ON search_runs (user_id) WHERE status = 'Running';

-- Earlier versions cached failed location lookups as "no location". Clear empty
-- results so they are looked up again; genuinely unknown places are re-cached.
DELETE FROM geocoding_cache WHERE results = '[]'::jsonb;
