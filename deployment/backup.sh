#!/bin/sh
set -eu
stamp=$(date -u +%Y%m%dT%H%M%SZ)
docker compose exec -T postgres pg_dump -U job_tracker -d job_tracker -Fc > "job-tracker-${stamp}.dump"
echo "Created job-tracker-${stamp}.dump"
