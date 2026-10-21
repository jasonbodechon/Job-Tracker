CREATE TABLE company_directory_preferences (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  company_key text NOT NULL,
  enabled boolean NOT NULL DEFAULT true,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(user_id, company_key)
);

-- Keep the healthiest directory record for each normalized company. A working
-- board is preferred over a stale one, then manual provenance, job coverage,
-- recent verification, and finally a stable ID tie-breaker.
WITH ranked AS (
  SELECT id,
         row_number() OVER (
           PARTITION BY lower(btrim(company_name))
           ORDER BY
             CASE verification_status
               WHEN 'Verified' THEN 0
               WHEN 'Redirected' THEN 1
               WHEN 'Provider change' THEN 2
               WHEN 'Pending' THEN 3
               WHEN 'Stale' THEN 4
               ELSE 5
             END,
             CASE identifier_origin WHEN 'Manual' THEN 0 ELSE 1 END,
             current_job_count DESC,
             last_verified_at DESC NULLS LAST,
             updated_at DESC,
             id
         ) AS duplicate_rank
  FROM company_directory
)
DELETE FROM company_directory
WHERE id IN (SELECT id FROM ranked WHERE duplicate_rank > 1);

CREATE UNIQUE INDEX company_directory_company_key_idx
ON company_directory(lower(btrim(company_name)));
