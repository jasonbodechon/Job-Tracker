ALTER TABLE users ADD COLUMN role text NOT NULL DEFAULT 'user' CHECK(role IN ('user','admin'));
ALTER TABLE company_directory ADD COLUMN enabled boolean NOT NULL DEFAULT true;
ALTER TABLE company_directory ADD COLUMN source_enabled boolean NOT NULL DEFAULT false;

-- Preserve directory sources that administrators had already added before the
-- site-wide administration migration.
UPDATE company_directory d SET source_enabled=true
WHERE EXISTS(SELECT 1 FROM sources s WHERE lower(btrim(s.company))=lower(btrim(d.company_name)));

INSERT INTO sources(user_id,id,kind,company,board_token,location,enabled)
SELECT u.id,
       'directory-' || d.id,
       CASE WHEN d.identifier_origin='Automatic' THEN COALESCE(d.detected_provider,d.ats_provider) ELSE d.ats_provider END,
       d.company_name,
       CASE WHEN (CASE WHEN d.identifier_origin='Automatic' THEN COALESCE(d.detected_provider,d.ats_provider) ELSE d.ats_provider END) IN ('Workday','CareerPage') THEN d.careers_url ELSE d.board_identifier END,
       CASE WHEN d.ats_provider='Adzuna' THEN d.countries->>0 ELSE NULL END,
       d.enabled
FROM users u CROSS JOIN company_directory d
WHERE d.source_enabled=true
ON CONFLICT(user_id,id) DO NOTHING;

-- Existing installations receive one administrator deterministically. On a
-- fresh installation the registration endpoint makes the first account admin;
-- later registrations remain users until an administrator promotes them.
UPDATE users SET role='admin'
WHERE id=(SELECT id FROM users ORDER BY created_at,id LIMIT 1);

CREATE INDEX users_role_idx ON users(role);
