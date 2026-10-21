ALTER TABLE sources
ADD COLUMN hydration_limit integer NOT NULL DEFAULT 0
CHECK (hydration_limit BETWEEN 0 AND 2000);

CREATE TABLE connector_detail_cache (
  provider text NOT NULL,
  external_id text NOT NULL,
  summary_fingerprint text NOT NULL,
  detail jsonb NOT NULL,
  hydrated_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(provider, external_id)
);

CREATE INDEX connector_detail_cache_last_used_idx
ON connector_detail_cache(last_used_at);
