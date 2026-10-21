ALTER TABLE profiles
  ADD COLUMN IF NOT EXISTS distance_enabled boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS active_location text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS active_latitude double precision,
  ADD COLUMN IF NOT EXISTS active_longitude double precision,
  ADD COLUMN IF NOT EXISTS maximum_distance_km double precision NOT NULL DEFAULT 75,
  ADD COLUMN IF NOT EXISTS include_remote boolean NOT NULL DEFAULT true,
  ADD COLUMN IF NOT EXISTS include_hybrid boolean NOT NULL DEFAULT true,
  ADD COLUMN IF NOT EXISTS include_onsite boolean NOT NULL DEFAULT true,
  ADD COLUMN IF NOT EXISTS broad_location_policy text NOT NULL DEFAULT 'review';

ALTER TABLE jobs
  ADD COLUMN IF NOT EXISTS location_candidates jsonb NOT NULL DEFAULT '[]',
  ADD COLUMN IF NOT EXISTS distance_km double precision,
  ADD COLUMN IF NOT EXISTS location_status text NOT NULL DEFAULT 'Not evaluated';

CREATE TABLE IF NOT EXISTS geocoding_cache (
  normalized_query text PRIMARY KEY,
  query text NOT NULL,
  results jsonb NOT NULL DEFAULT '[]',
  resolved_at timestamptz NOT NULL DEFAULT now()
);

UPDATE profiles
SET weights = weights || '{"proximity":10}'::jsonb
WHERE NOT (weights ? 'proximity');

ALTER TABLE profiles DROP CONSTRAINT IF EXISTS profiles_distance_range_check;
ALTER TABLE profiles ADD CONSTRAINT profiles_distance_range_check CHECK (maximum_distance_km > 0 AND maximum_distance_km <= 2000);
ALTER TABLE profiles DROP CONSTRAINT IF EXISTS profiles_broad_policy_check;
ALTER TABLE profiles ADD CONSTRAINT profiles_broad_policy_check CHECK (broad_location_policy IN ('review','exclude'));
