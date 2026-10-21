UPDATE profiles
SET distance_enabled = true,
    updated_at = now();

ALTER TABLE profiles ALTER COLUMN distance_enabled SET DEFAULT true;
