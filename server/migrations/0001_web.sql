CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  email text NOT NULL UNIQUE,
  display_name text NOT NULL,
  password_hash text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE sessions (
  token_hash text PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user_idx ON sessions(user_id);

CREATE TABLE profiles (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  id text NOT NULL, name text NOT NULL, titles jsonb NOT NULL DEFAULT '[]',
  required_skills jsonb NOT NULL DEFAULT '[]', preferred_skills jsonb NOT NULL DEFAULT '[]',
  remote_only boolean NOT NULL DEFAULT true, countries jsonb NOT NULL DEFAULT '[]',
  regions jsonb NOT NULL DEFAULT '[]', cities jsonb NOT NULL DEFAULT '[]', min_score integer NOT NULL DEFAULT 55,
  enabled boolean NOT NULL DEFAULT true, schedule text NOT NULL DEFAULT 'Manual', resume_text text NOT NULL DEFAULT '',
  weights jsonb NOT NULL DEFAULT '{"title":25,"requiredSkills":40,"preferredSkills":15,"remote":15,"freshness":5}',
  next_run_at timestamptz, updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(user_id,id)
);
CREATE TABLE sources (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  id text NOT NULL, kind text NOT NULL, company text NOT NULL, board_token text NOT NULL,
  location text, enabled boolean NOT NULL DEFAULT true, credentials_ciphertext text,
  last_refreshed_at timestamptz, last_error text, updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(user_id,id)
);
CREATE TABLE jobs (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  id text NOT NULL, source text NOT NULL, external_id text NOT NULL, source_id text,
  title text NOT NULL, company text NOT NULL, location text NOT NULL, remote_type text NOT NULL,
  posted_at text NOT NULL, apply_url text NOT NULL, description text NOT NULL, skills jsonb NOT NULL DEFAULT '[]',
  match_score integer NOT NULL, match_reason text NOT NULL, profile_matches jsonb NOT NULL DEFAULT '[]',
  is_new boolean NOT NULL DEFAULT true, active boolean NOT NULL DEFAULT true, status text NOT NULL DEFAULT 'New',
  first_seen_at timestamptz NOT NULL DEFAULT now(), last_seen_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(user_id,id)
);
CREATE INDEX jobs_user_active_idx ON jobs(user_id,active);
CREATE TABLE trackers (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, job_id text NOT NULL,
  status text NOT NULL DEFAULT 'New', notes text NOT NULL DEFAULT '', contact_name text NOT NULL DEFAULT '', contact_email text NOT NULL DEFAULT '',
  application_date text, interview_date text, follow_up_at text, updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(user_id,job_id), FOREIGN KEY(user_id,job_id) REFERENCES jobs(user_id,id) ON DELETE CASCADE
);
CREATE TABLE search_runs (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, id text NOT NULL,
  started_at text NOT NULL, finished_at text, trigger_type text NOT NULL, sources_checked integer NOT NULL DEFAULT 0,
  jobs_imported integer NOT NULL DEFAULT 0, errors jsonb NOT NULL DEFAULT '[]', PRIMARY KEY(user_id,id)
);
CREATE TABLE notifications (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title text NOT NULL, body text NOT NULL, url text NOT NULL DEFAULT '/', read_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE push_subscriptions (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  endpoint text NOT NULL UNIQUE, p256dh text NOT NULL, auth text NOT NULL, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE schema_imports (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  source_format text NOT NULL, imported_at timestamptz NOT NULL DEFAULT now(), summary jsonb NOT NULL
);
