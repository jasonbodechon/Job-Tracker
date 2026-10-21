ALTER TABLE sources ADD COLUMN request_delay_ms integer NOT NULL DEFAULT 0 CHECK (request_delay_ms BETWEEN 0 AND 60000);
ALTER TABLE sources ADD COLUMN max_retries integer NOT NULL DEFAULT 0 CHECK (max_retries BETWEEN 0 AND 10);
ALTER TABLE sources ADD COLUMN max_results integer NOT NULL DEFAULT 0 CHECK (max_results BETWEEN 0 AND 20000);
ALTER TABLE sources ADD COLUMN page_size integer NOT NULL DEFAULT 0 CHECK (page_size BETWEEN 0 AND 500);
ALTER TABLE sources ADD COLUMN max_calls integer NOT NULL DEFAULT 0 CHECK (max_calls BETWEEN 0 AND 500);
