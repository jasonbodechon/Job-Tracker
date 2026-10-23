# Job-Tracker

Current version: 0.20.0

Job Tracker is now a self-hosted web application and installable PWA. The React client uses a Rust/Axum API, PostgreSQL, and a separate Rust worker. The v0.8.1 Tauri source remains in `src-tauri` during the migration window, and the web Settings page imports its SQLite database directly.

## Migrated features

- Account registration/login with Argon2 password hashing, revocable HttpOnly sessions, and strict per-user database ownership.
- Greenhouse, Lever, Ashby, and USAJOBS connectors executed only by the server. USAJOBS credentials are encrypted with AES-GCM.
- Multiple always-active profiles, configurable weights, multi-profile scoring, source/profile/status filters, date ranges, and sortable result columns.
- Scoring Engine v2 analyzes resume text and job descriptions, recognizes common skill aliases and phrases, compares detected experience, omits inapplicable categories from the denominator, and shows a component-by-component explanation in job details.
- Distance profiles can include every eligible remote job plus hybrid/on-site jobs within a configurable kilometre radius. City/address geocoding is cached, multiple offices use the nearest location, and province/state/country-only postings remain visible as location-review results without guessed centroid distances.
- Full job detail and application tracker including notes, contacts, application/interview dates, and follow-ups.
- Manual and per-source refreshes plus daily, weekday, and weekly searches executed by the worker while browsers are closed.
- In-app notification records and optional standards-based Web Push through the PWA service worker.
- CSV export, portable JSON archive export, direct v0.8.1 SQLite import, diagnostics, and retention cleanup.
- Docker deployment with PostgreSQL, API, worker, automatic migrations, Caddy TLS, persistent volumes, and a database backup script.

## Production deployment

Requirements: Docker Engine with Compose, a public DNS name pointing to the server, and inbound TCP ports 80 and 443 open at the host/firewall.

1. Copy `.env.example` to `.env`.
2. Set independent, randomly generated values for `POSTGRES_PASSWORD` and `ENCRYPTION_KEY`.
3. Set `JOB_TRACKER_DOMAIN` and `APP_ORIGIN` to the same public hostname, for example `jobs.example.com` and `https://jobs.example.com`. Do not use an IP address, `localhost`, or a self-signed certificate for production.
4. Generate a VAPID key pair with a standards-compatible Web Push key generator and put the URL-safe public/private values in `.env`. Push is optional; in-app notification history works without it.
5. Run `docker compose up -d --build`. Caddy validates the public DNS name, automatically obtains a trusted certificate from a public certificate authority, redirects HTTP to HTTPS, and renews the certificate.
6. Confirm `https://your-domain/api/health` returns `{"status":"ok"}` and create the first account.

Caddy obtains and renews TLS certificates for a public domain. It will now refuse to start the production Compose configuration when `JOB_TRACKER_DOMAIN` is missing, avoiding an accidental local/self-signed deployment.

For a complete local Docker run without a certificate, use the dedicated localhost-only override:

```bash
docker compose -f docker-compose.yml -f docker-compose.local.yml up -d --build
```

Open `http://localhost:8080`. This local override sets `COOKIE_SECURE=false` and is intentionally not suitable for a LAN or public deployment. For non-Docker frontend development, set `COOKIE_SECURE=false`, `APP_ORIGIN=http://localhost:1420`, run PostgreSQL, then start `cargo run -p job-tracker-web --bin job-tracker-api`, `cargo run -p job-tracker-web --bin job-tracker-worker`, and `npm run dev` in separate terminals.

`Start Job Tracker Local.cmd` runs the full local setup: it installs missing prerequisites, checks that Node.js is version 20 or newer, runs the frontend tests, rebuilds the Docker images (which build the frontend and run the Rust tests), and starts the app. When an existing local database is found, it first saves a backup to `backups\job-tracker-YYYYMMDD-HHMMSS.dump` and stops without upgrading if the backup fails. The `backups/` folder is ignored by git and Docker builds; delete old backups yourself when you no longer need them. Restore one with `docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml exec -T postgres pg_restore -U job_tracker -d job_tracker --clean --if-exists < backups\<file>.dump`.

Job searches run in the background on the server. **Search now** starts a search and the page shows its progress until it finishes; reloading or closing the page does not stop it, and a reloaded page picks the running search back up. Only one search runs at a time for each user. Outbound requests time out after 60 seconds (10 seconds to connect, 15 seconds per location lookup), failed location lookups are retried on the next search instead of being cached, and a search that stops reporting progress for 30 minutes, or was interrupted by a restart, is marked as abandoned. The first search after a fresh install is the slowest, because each new job location is looked up at most once per second and then cached.

Run `"Start Job Tracker Local.cmd" quick` to start an existing installation without reinstalling packages, testing, rebuilding, or backing up. The package security audit only reports high or critical findings; startup never changes `package.json` or `package-lock.json`. A new `.env.local` gets 32-byte random secrets generated by `deployment/local-tools.mjs`. Keep double quotes and exclamation marks out of `POSTGRES_PASSWORD`, because CMD cannot pass them safely.

`Start Job Tracker Local.cmd` keeps the password in the persistent local PostgreSQL volume synchronized with `.env.local` before the health check. This prevents a regenerated `.env.local` from blocking the API while preserving existing local profiles, sources, and job data. It applies only to the localhost configuration; production credentials are never changed automatically.

Create a PostgreSQL backup with `sh deployment/backup.sh`. Test restoration regularly in a non-production database.

## Desktop data migration

1. Close Job Tracker v0.8.1 so SQLite has flushed its WAL.
2. Locate the desktop database shown in its Settings diagnostics (`job-tracker.sqlite3`). Keep an untouched copy.
3. Sign in with an administrator account, open **Admin settings → Data**, choose **Import or restore**, and select that file.
4. Review the displayed profile/source/job totals. USAJOBS credentials are intentionally not imported; re-enter them in Sources.
5. Run a manual search and compare profile counts, job counts, statuses, and several match explanations before uninstalling the desktop app.

Imports are merge/upsert operations scoped to the signed-in user. Re-importing the same database updates matching IDs rather than creating duplicate records.

## Verification

Frontend:

```bash
npm ci
npm run test:frontend
npm run build
```

Server (requires Rust):

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Deployment smoke test:

```bash
docker compose up -d --build
docker compose exec -T postgres pg_isready -U job_tracker -d job_tracker
curl --fail https://your-domain/api/health
```

Use `Start Job Tracker Local.cmd` for a localhost Docker run or `Deploy Job Tracker Production.cmd` for a public trusted-HTTPS deployment. Both CMD files install missing Docker Desktop and Node.js LTS through Windows Package Manager, run frontend tests, build the Docker images (including Rust workspace tests), wait for health, and open the appropriate browser URL. The production script also runs the audit repair rule and a separate frontend build; the local script only reports audit findings and backs up the database before upgrading. The retired Windows installer, uninstaller, and verification scripts have been removed; `src-tauri` remains only as v0.8.1 migration source.

## Upgrades and Docker storage

Extract a new Job Tracker release while retaining `.env` or `.env.local`, then run the matching Production or Local CMD again. Each CMD detects whether an API, worker, PostgreSQL volume, or Caddy data volume already exists. With no previous Job Tracker state it performs a `NEW INSTALL`; otherwise it performs an `UPDATE`, recreating missing or changed services while retaining data. A new installation uses the stable Compose project name `job-tracker`. The upgrade detector can also adopt one legacy Job Tracker PostgreSQL volume created under a versioned folder name, preventing a new extraction directory from hiding the existing database.

The scripts pull current PostgreSQL and Caddy images, rebuild the API/PWA/worker with current base images, run all tests, and use `docker compose up -d --remove-orphans` to install or replace containers. PostgreSQL and Caddy named volumes remain attached, so application data and certificates survive upgrades. If multiple legacy Job Tracker PostgreSQL volumes exist, startup stops instead of guessing which database to use.

Before a local install or update, the Local CMD checks port 8080. An older Job-Tracker Caddy container holding that port is stopped and replaced without deleting its volumes. A non-Job-Tracker port owner is reported by name and is never stopped automatically.

Do not delete Docker volumes during a normal upgrade. In particular, do not run `docker compose down -v`, because `-v` deletes the PostgreSQL data volume. Old stopped containers and dangling images are not required by the new version and may be pruned later to reclaim disk space, but cleanup is optional and should happen only after the upgraded application is healthy and a database backup has been verified.

Scoring Engine v2 is installed through a PostgreSQL migration. Existing active jobs are automatically rescored when results are loaded, after a profile is saved or deleted, and after a search. Legacy remote/freshness score weights are replaced by the v2 defaults; remote and location remain eligibility filters rather than score boosters.

### Distance-based profiles

Open Profiles and find **Distance and work-style eligibility** directly below the resume parser. Distance eligibility is always active: enter a city/address, maximum kilometres, included work styles, and the broad-location policy, then save. Remote jobs do not receive or require proximity points. Confirmed hybrid/on-site locations must fall inside the range, while the optional proximity weight ranks nearer eligible jobs more highly. Search results provide location-status filtering, distance sorting, and a separate distance/status view.

Every saved profile is automatically included in manual and scheduled searches. The retired **Remote only**, **Allowed job locations**, and **Include in searches** controls are no longer shown or applied. Upgrade migration `0004` activates existing profiles and clears the retired location-filter values so hidden legacy settings cannot affect results.

Version 0.12.1 also removes the distance enable/disable flag, dismisses application status messages after 30 seconds, keeps fit-weight inputs aligned when their labels wrap, uses DM Sans for all page and card headings, and fixes the application shell so the left navigation remains in the viewport while the main content scrolls.

Version 0.13.0 adds an authenticated Company directory with company search, Canada/United States and ATS-provider filters, board health, job counts, verification history, provenance, and one-click source creation. The worker verifies due entries in small batches, marks failures stale, records redirects/provider changes, and discovers directory records from saved sources without overwriting curated manual identifiers.

Source coverage now includes Amazon Jobs, SmartRecruiters, generic Workday tenants, Microsoft Careers, Google Careers, structured career pages exposing `JobPosting` JSON-LD, and optional Adzuna searches. All adapters emit the same normalized posting contract used by location eligibility, profile/resume scoring, job sorting, and tracking. Connector labels describe the acquisition method: Greenhouse, Lever, Ashby, SmartRecruiters, Adzuna, and USAJOBS are **Official API**; Amazon, Workday, and Microsoft are **Public site feed**; Google and structured career pages are **Public page extraction**. Public feeds and page extraction can become stale when a proprietary site changes and are intentionally isolated behind their connectors.

Adzuna is optional. Add `ADZUNA_APP_ID` and `ADZUNA_APP_KEY` to the active `.env` or `.env.local` file before enabling an Adzuna source. The Start CMD upgrade flow passes these values to the API and worker through the existing environment file automatically.

Version 0.13.1 adds multi-page importing for Amazon Jobs, SmartRecruiters, Workday, Adzuna, and USAJOBS. Connectors follow the provider's reported total, tolerate provider-specific page-size caps, stop on empty pages, deduplicate results across pages, and retain the full reported count for the Company directory. `CONNECTOR_MAX_RESULTS` limits one source search to 5,000 jobs by default and accepts values from 100 through 20,000. Directory verification deliberately samples only the first 100 results so routine health checks do not download entire boards.

Version 0.13.2 removes redundant `final_url` initialization from all paginated connectors. Pagination now explicitly retains the first successful response's final URL for Company-directory provider and redirect analysis, rather than repeatedly replacing it with later page URLs.

Version 0.14.0 identifies equivalent directory sources by normalized company, ATS provider, and board identifier instead of relying only on a generated row ID. Re-adding an equivalent source re-enables the existing record, while **Remove source** on the Company directory deletes every saved equivalent so older duplicates are cleaned up together.

Google Careers has a dedicated paginated public-page adapter for its server-rendered job cards. Microsoft Careers uses a dedicated paginated adapter for the public feed behind its current Eightfold-powered application site. Both are provider-owned public-site contracts rather than documented ATS APIs and may require maintenance when those sites change; Adzuna remains an optional supplementary coverage source.

Version 0.14.1 attempted to restore the already-published `0006_company_directory.sql` migration, but the historical file still differed from the checksum stored by some existing installations. It is superseded by the guarded recovery in version 0.14.2. The Microsoft URL update remains isolated in migration `0007`.

Version 0.14.2 adds a narrowly scoped startup recovery for installations affected by the migration 6 packaging mistake. Before running normal migrations, the API and worker repair migration 6's recorded checksum only when SQLx shows that migration succeeded, its description matches the embedded company-directory migration, and every expected `company_directory` column exists. Fresh installations and databases whose migration 6 identity or schema do not match are not modified. Migration 7 then applies normally.

Version 0.14.3 keeps **Remove source** visible for every Company directory row. It is enabled when an equivalent saved source exists and disabled with an explanatory tooltip otherwise. A disabled saved source shows both **Enable source** and **Remove source**, so it can either be restored or deleted directly from the directory.

Version 0.15.0 consolidates duplicate Company directory rows during migration `0008`. For each normalized company name, Job-Tracker keeps the best record by preferring healthy verification status, manual provenance, greater current-job coverage, newer verification, and a stable ID tie-breaker. A unique normalized-company index and discovery guard prevent the background worker from recreating duplicates.

Version 0.16.0 introduces administrator and user roles. The first account on a fresh installation becomes an administrator; an upgraded installation promotes its oldest account deterministically. Later accounts default to user. Administrators can change other users' roles, but cannot remove their own administrator role.

**Admin settings** is now the only location for the Company directory, Sources, and data management/diagnostics. Its tabs provide site overview metrics, Company directory search and filters, company add/edit/delete and verification, site-wide source management, a global schedule control, role management, exports/imports, site-wide retention, and site-wide diagnostics. Profiles remain exclusively user-managed. Account archive/CSV operations remain scoped to the signed-in administrator; use `deployment/backup.sh` for a complete PostgreSQL backup. Personal profile metrics and Best job matches are hidden while an administration tab is open. The old Settings and standalone Company directory navigation buttons are removed.

SmartRecruiters, generic Workday, and optional Adzuna are available in the administrator company editor and use their existing normalized connectors. SmartRecruiters accepts its public company identifier; Workday requires the public `myworkdayjobs.com` tenant/site URL; Adzuna uses the board identifier as a search keyword and requires `ADZUNA_APP_ID` and `ADZUNA_APP_KEY` in the active environment file.

Company directory enable/disable state and directory source actions are now site-wide. Disabling a company excludes its matching sources from manual and scheduled searches for every account and hides its active jobs while preserving jobs, trackers, and history. **Add source** replaces outdated equivalents and provisions the selected connector for all current users; new users inherit directory sources that an administrator selected with **Add source**. **Remove source** removes the equivalent source from every account. Deleting a company removes its sources and retires its active jobs globally while retaining job and tracker records.

Version 0.16.1 removes the duplicate Dashboard and makes **Job matches** the default landing page. Statistics, Best job matches, and search activity render only on Job matches; the Profiles page keeps its profile editor and active-profile summary without result metrics, while Schedules shows only scheduling controls and notifications. Sources no longer appears in user navigation and is available only through **Admin settings → Sources**. Administrator source changes are synchronized to every existing account, migration `0010` consolidates legacy sources across users, and newly registered users inherit the current source set.

Version 0.17.0 adds provider-aware connector throttling, exponential retries, Retry-After handling, maximum-call and result safety limits, partial-page preservation, diagnostic error codes with secret redaction, and site-wide per-source tuning controls. Amazon refreshes use a dedicated keyword when supplied and conservative automatic defaults. Migration `0011` adds the connector settings without modifying any prior migration.

Version 0.18.0 moves the Active profile card above Resume and search profiles, removes unnecessary trailing zeroes from fit-component scores, and exposes editable request controls under every saved source in **Admin settings → Sources**. The Google parser now recognizes current relative job links, accessible titles, and result totals. Microsoft uses a dedicated paginated adapter for the public Eightfold site feed. Migration `0012` reclassifies Microsoft and schedules it for verification without modifying a historical migration.

Version 0.19.0 updates the Microsoft connector for Eightfold's current `data` response envelope and adds response-contract checks with explicit search and detail error codes. New or changed Microsoft postings are hydrated from the position-detail feed, while unchanged postings reuse a shared PostgreSQL detail cache. A SHA-256 fingerprint of stable summary fields determines whether a detail must be downloaded again. Hydration uses the source's request delay and retry policy, is capped by a configurable **Hydration limit** (automatic default: 50 requests per refresh), preserves cached detail during temporary failures, and progressively fills the cache over later searches. Microsoft's automatic request policy is a one-second delay with four retries because the public site can return HTTP 429 during bursts. Unused cache records expire after 90 days. Migration `0013` adds the source setting and cache table without modifying prior migrations.

Version 0.19.1 corrects the Microsoft millisecond timestamp test fixture to use an explicit Rust `i64`, preventing the Docker `cargo test` build from rejecting the value as an overflowing `i32`. Runtime connector behavior and migration `0013` are unchanged.

Version 0.20.0 adds severity-aware Company Directory status badges in **Admin settings → Companies**: Verified is green, Stale is yellow, Pending is orange, and every other status—including Redirected and Provider change—is red. Statuses with diagnostic data expose an accessible mouse-hover and keyboard-focus popup containing the status, extracted connector error code, complete error message, detected provider, and redirect destination when available.

Connector controls are under **Admin settings → Sources**. Enter `0` to use the automatic provider default. The automatic defaults are Amazon `250 ms / 4 retries / 100 per call / 1,000 total / 10 calls`; USAJOBS `100 ms / 3 / 250 / 2,500 / 10`; SmartRecruiters `125 ms / 3 / 100 / 2,000 / 20`; Workday and Adzuna `250 ms / 3 / 20 / 1,000 / 50`; and Google `500 ms / 3 / 100 / 1,000 / 20`. One-call board APIs and page extractors use three retries. `CONNECTOR_MAX_RESULTS` remains the server-wide ceiling over each source's configured total.

The administrator-controlled site-wide schedule is stored separately in `site_settings`. Users see its current value in a read-only card at the top of Schedules while retaining their individual profile schedule controls. Applying a new site-wide schedule from Admin settings updates every existing profile and the read-only value together.

The API caches geocoding responses in PostgreSQL. `GEOCODING_URL` defaults to the OpenStreetMap Nominatim endpoint and `GEOCODING_DELAY_MS=1000` limits uncached request frequency. Production operators may point `GEOCODING_URL` at a compatible managed or self-hosted endpoint. Broad locations are never mapped to a province/state/country centroid.

## Security and operations

- Serve only through HTTPS in production and leave `COOKIE_SECURE=true`. A trusted Caddy-managed certificate is required for the PWA and reliable Web Push; a self-signed certificate is for isolated testing only.
- Back up PostgreSQL and the `.env` secrets separately. Losing `ENCRYPTION_KEY` makes stored USAJOBS credentials unrecoverable.
- Rotate sessions by deleting rows in `sessions`; passwords and raw session tokens are never stored.
- API CORS is restricted to `APP_ORIGIN`; state-changing requests require JSON and authenticated SameSite cookies.
- The upload importer is capped at 100 MB, opens SQLite read-only, and selects a fixed set of known columns.
- Apply dependency and container updates regularly and run the full verification matrix before deployment.

## Architecture

- `src/`: React/TypeScript PWA and authenticated HTTP client.
- `server/src/bin/api.rs`: Axum API and static web host.
- `server/src/bin/worker.rs`: background schedules, connector refreshes, and follow-up notifications.
- `server/migrations/`: PostgreSQL schema and indexes.
- `src-tauri/`: retained v0.8.1 desktop source for the migration window.
- `deployment/`: TLS proxy and backup assets.
