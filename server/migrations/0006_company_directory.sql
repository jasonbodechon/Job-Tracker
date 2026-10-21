CREATE TABLE company_directory (
  id text PRIMARY KEY,
  company_name text NOT NULL,
  countries jsonb NOT NULL DEFAULT '[]',
  ats_provider text NOT NULL,
  connector_class text NOT NULL,
  board_identifier text NOT NULL,
  careers_url text NOT NULL,
  current_job_count integer NOT NULL DEFAULT 0,
  last_verified_at timestamptz,
  next_verification_at timestamptz NOT NULL DEFAULT now(),
  verification_status text NOT NULL DEFAULT 'Pending',
  verification_error text,
  identifier_origin text NOT NULL DEFAULT 'Automatic',
  detected_provider text,
  redirect_url text,
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX company_directory_verification_idx ON company_directory(next_verification_at);
CREATE INDEX company_directory_provider_idx ON company_directory(ats_provider);

INSERT INTO company_directory(id,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,identifier_origin)
VALUES
  ('amazon','Amazon','["Canada","United States"]','Amazon','Public site feed','amazon','https://www.amazon.jobs/en/search','Manual'),
  ('google','Google','["Canada","United States"]','Google','Public page extraction','google','https://www.google.com/about/careers/applications/jobs/results/','Manual'),
  ('microsoft','Microsoft','["Canada","United States"]','Microsoft','Public page extraction','microsoft','https://jobs.careers.microsoft.com/global/en/search','Manual')
ON CONFLICT(id) DO NOTHING;
