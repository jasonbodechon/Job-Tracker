UPDATE profiles
SET enabled = true,
    remote_only = false,
    countries = '[]'::jsonb,
    regions = '[]'::jsonb,
    cities = '[]'::jsonb,
    updated_at = now();

ALTER TABLE profiles ALTER COLUMN enabled SET DEFAULT true;
