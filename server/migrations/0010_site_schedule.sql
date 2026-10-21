CREATE TABLE site_settings (
  key text PRIMARY KEY,
  value text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO site_settings(key,value)
SELECT 'global_schedule',COALESCE((
  SELECT schedule FROM profiles
  GROUP BY schedule
  ORDER BY count(*) DESC,schedule
  LIMIT 1
),'Manual');

-- Sources are administrator-managed from this release onward. Choose the most
-- recently updated copy of each legacy source and synchronize it to all users.
WITH canonical AS (
  SELECT DISTINCT ON(id) id,kind,company,board_token,location,enabled,credentials_ciphertext,last_refreshed_at,last_error
  FROM sources
  ORDER BY id,updated_at DESC
)
INSERT INTO sources(user_id,id,kind,company,board_token,location,enabled,credentials_ciphertext,last_refreshed_at,last_error)
SELECT u.id,c.id,c.kind,c.company,c.board_token,c.location,c.enabled,c.credentials_ciphertext,c.last_refreshed_at,c.last_error
FROM users u CROSS JOIN canonical c
ON CONFLICT(user_id,id) DO UPDATE SET
  kind=excluded.kind,company=excluded.company,board_token=excluded.board_token,
  location=excluded.location,enabled=excluded.enabled,
  credentials_ciphertext=COALESCE(excluded.credentials_ciphertext,sources.credentials_ciphertext),
  last_refreshed_at=excluded.last_refreshed_at,last_error=excluded.last_error,updated_at=now();
