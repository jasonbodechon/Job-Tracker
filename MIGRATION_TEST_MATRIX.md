# Job Tracker Web Migration Test Matrix

Status date: 2026-08-21. Frontend TypeScript, 67 unit tests, and the production PWA build passed in the implementation workspace. Rust commands are included below but were not executable there because the Rust toolchain was unavailable.

| Step | Feature | Automated coverage | Acceptance test | Implementation |
|---:|---|---|---|---|
| 1 | Shared domain, matching, connectors | Rust scoring/location unit test; existing TS normalization/matching suites | Compare a fixed Greenhouse/Lever/Ashby/USAJOBS fixture with v0.8.1 scores and explanations | Complete |
| 2 | Axum HTTP API | Typed client unit tests; route/auth code review | Register, sign in, CRUD profiles/sources/jobs/trackers, sign out, confirm 401 afterward | Complete; compile in Docker/Rust CI required |
| 3 | PostgreSQL | Migration constraints and per-user composite keys | Migrate empty Postgres, create two users, verify neither can read/update the other's IDs | Complete; live Postgres test required |
| 4 | Browser API client | Vitest request method/body/credentials/encoding tests | Use every UI save/refresh/export operation and inspect responses | Complete |
| 5 | Authentication/security | Password/session/ownership implementation review | Test bad login, duplicate email, expired session, Secure cookie, cross-account ID access | Complete; penetration/rate-limit review recommended |
| 6 | Background schedules | Worker scheduling and notification paths | Close all browsers, wait for due profile, verify search run and next-run timestamp | Complete; clock-based deployment test required |
| 7 | PWA/Web Push | Service worker build and Push API path | Install PWA, grant permission, run worker notification with browser closed, click notification | Complete; requires production HTTPS/VAPID test |
| 8 | Desktop migration/import/export | Multipart client test; read-only fixed-column importer test | Import a copied v0.8.1 SQLite DB and compare profiles, sources, jobs, statuses, notes, contacts, and dates | Complete |
| 9 | Regression/equivalence tests | 67 frontend tests plus Rust unit tests authored | Run `npm run test:frontend`, `cargo test --workspace`, cross-user tests, connector fixtures | Frontend passed; Rust execution pending toolchain |

## Company directory and connector matrix

| Feature | Automated coverage | Acceptance check | Status |
|---|---|---|---|
| Directory fields | TypeScript contracts and production build | Confirm company, countries, provider, identifier, count, verification date/status, and actions render | Complete |
| Company search | Case-insensitive filter unit test | Search with partial mixed-case company name | Complete |
| Country filters | Canada/United States OR-filter unit test | Select one and both countries | Complete |
| ATS filter | Combined country/provider unit test | Select multiple providers and combine with a country | Complete |
| Add source | Stable source-ID unit test; authenticated API route | Add once, reload, and confirm the button reads Added | Complete; deployment smoke test required |
| Background verification | Worker hook, due-batch query, success/stale state paths | Run worker against valid and invalid boards and inspect timestamps/count/status | Complete; live endpoint test required |
| Redirect/ATS detection | Provider URL classifier Rust test | Redirect a test board to another supported provider and confirm Provider change | Complete; live endpoint test required |
| Provenance isolation | Manual/automatic upsert constraints and discovery implementation | Save a source matching a curated company and confirm the manual row is unchanged | Complete; live PostgreSQL test required |
| Amazon Jobs | Normalization Rust fixture | Fetch a query and confirm normalized fields and reported count | Complete; live feed smoke test required |
| SmartRecruiters | Normalized connector and official endpoint adapter | Fetch a public company identifier and open an imported job | Complete; live API smoke test required |
| Generic Workday | Tenant/site validation and CXS feed adapter | Add a full `myworkdayjobs.com` site URL and import jobs | Complete; tenant compatibility smoke test required |
| Microsoft Careers | Isolated embedded-data/structured-page extractor | Verify seeded Microsoft row and inspect extraction status | Complete; extraction maintenance may be required when its site changes |
| Google Careers | Isolated embedded-data/structured-page extractor | Verify seeded Google row and inspect extraction status | Complete; extraction maintenance may be required when its site changes |
| JSON-LD career page | `JobPosting` extraction Rust fixture | Add an HTTPS page containing one or more JSON-LD job records | Complete |
| Adzuna | Credential checks and normalized results adapter | Configure both optional keys, search Canada and United States separately | Complete; credentials/live API test required |
| Normalized pipeline | Every connector returns the shared Rust `Posting` type | Import one job/provider and confirm location, scoring, sorting, matching, and tracking | Complete; provider integration matrix required |
| Expanded pagination | Rust stop/continue tests cover totals, short provider pages, empty pages, and configured limits; frontend test covers truncation notice | Import a board with more than 100 jobs and compare fetched count to its reported total or configured cap | Complete; live provider smoke test required |
| Final response URL retention | Rust regression test confirms later pagination pages cannot replace the first response URL | Build with Rust warnings enabled and verify directory provider detection after a redirected first page | Complete; Docker compiler verification required |

## Scoring Engine v2 matrix

| Item | Automated coverage | Acceptance check | Status |
|---:|---|---|---|
| 1. Scoring Engine v2 | Versioned score records; applicable-category denominator test | A job satisfying every applicable category scores 100 even when unrelated categories are absent | Complete |
| 2. Resume analysis | Resume keyword/phrase evidence and dated employment-range tests | Upload a resume containing skills and dated roles; open a result and inspect Resume evidence and estimated years | Complete |
| 3. Job-description analysis | Required years, skill phrases, education, and certification extraction tests | Open jobs with and without explicit requirements and compare displayed score components | Complete |
| 4. Phrase and alias matching | PostgreSQL/Postgres and AWS/Amazon Web Services Rust tests | Configure canonical terms and verify jobs using aliases match | Complete |
| 5. Experience comparison | Full and insufficient-experience Rust tests | Compare a 5-year requirement against longer and shorter resume histories | Complete |
| 6. Transparent breakdown | Typed API model plus TypeScript render/build validation | Open job details and verify earned/possible points and explanations per profile | Complete |
| 7. Automatic rescoring | Migration version marker and rescore paths on load, profile save/delete, and search | Upgrade with saved jobs, load results, and verify current scoring-engine data without deleting volumes | Complete |

## Distance-profile matrix

| Feature | Automated coverage | Acceptance check | Status |
|---|---|---|---|
| Remote eligibility | Rust tests verify distance is omitted for remote jobs | Enable distance mode and confirm unrestricted remote jobs remain visible | Complete |
| Radius eligibility | Haversine near/far Rust tests | Set Kitchener with 75 km and compare Waterloo against Toronto | Complete |
| Multiple offices | Candidate extraction test | Use `Toronto or Waterloo`; confirm the nearest resolved office is selected | Complete |
| Broad locations | Province/country classification and review/exclude tests | Compare both broad-location policy options without centroid distances | Complete |
| Proximity weighting | Rust score-component test coverage | Compare two eligible non-remote jobs at different distances | Complete |
| Result controls | TypeScript filter/sort regression test | Filter by location status and sort ascending/descending distance | Complete |
| Upgrade/rescore | Migration plus scoring-version invalidation | Run the CMD update and verify existing active jobs are reevaluated | Complete |
| Consolidated profile UI | Source regression test verifies the single distance section follows the resume parser | Open Profiles and confirm no separate distance card appears | Complete |
| Always-active profiles | UI/API/service regression tests plus migration `0004` | Create multiple profiles and confirm all participate in manual and scheduled searches | Complete |
| Retired filters | Source regression test verifies Remote only, Allowed job locations, and Include in searches are absent | Upgrade an older database and confirm legacy values do not silently filter jobs | Complete |
| Permanent distance mode | UI/API/default-profile checks plus migration `0005` | Upgrade and confirm distance fields remain visible with no enable/disable flag | Complete |
| Timed status messages | Source regression verifies a 30-second reset timer | Save a profile/source and confirm its notice disappears after 30 seconds | Complete |
| Weight alignment | Wrapped-label markup and CSS regression checks | Resize the Profiles card and confirm all weight inputs remain aligned | Complete |
| Fixed navigation | Viewport shell and independently scrollable-content CSS checks | Scroll a long page and confirm the left navigation remains visible | Complete |
| Heading typography | CSS regression excludes Fraunces and applies DM Sans to heading levels | Compare Welcome, Job matches, cards, and modal headings | Complete |
| 10 | Deployment/coexistence | Production Vite build passed; container definitions reviewed | `docker compose up -d --build`, health check, TLS, backup/restore drill, worker logs | Complete; deployment smoke test pending host |

## Feature regression matrix

| Area | Cases |
|---|---|
| Profiles | create/update/delete; comma entry; multiple values; resume PDF/DOCX/TXT; weights; minimum score; inline distance/work-style controls; every profile always active |
| Sources | all eleven providers; invalid board; disabled source; per-source refresh; all-source refresh; USAJOBS encrypted credentials; optional Adzuna credentials; provider error persisted |
| Results | all column sorts both directions; text search; multi-source/profile/status filters; inclusive date bounds; details modal; application URL |
| Tracker | each status; notes/contact fields; application/interview/follow-up dates; due count; archive/reject behavior |
| Schedules | Manual/Daily/Weekdays/Weekly; browser closed; duplicate-run prevention; provider failure notification; follow-up notification deduplication |
| Data | CSV download; JSON archive round trip; desktop SQLite merge; repeat import; 100 MB rejection; retention boundaries; PostgreSQL backup restore |
| Security | password length/hash; no raw session tokens; logout/revocation; cookie flags; exact CORS origin; account isolation; credential ciphertext; upload table/column allowlist |
| PWA | manifest/install; offline shell; API excluded from cache; Push permission denied/granted; notification click; service-worker cache upgrade |

## Administration matrix

| Feature | Automated coverage | Acceptance check | Status |
|---|---|---|---|
| Roles and authorization | Migration/source tests cover first-admin promotion, default role, protected routes, and self-demotion guard | Upgrade an existing database, confirm the oldest account is admin, then promote/demote a second account | Complete; Docker/PostgreSQL integration test pending |
| Simplified navigation | Source regression checks one conditional Admin settings entry, no Dashboard, and no user Sources navigation | Sign in as user and admin and compare navigation | Complete |
| Site-wide companies | Source regression covers global add/remove and new-user synchronization | Add, disable, enable, replace, and remove one company while signed into two accounts | Complete; Docker/PostgreSQL integration test pending |
| Company connectors | Typed API test covers administrator company save; existing adapter tests cover normalized connectors | Verify live SmartRecruiters, Workday, and credentialed Adzuna entries | Complete; live provider smoke tests pending |
| Company search/filter | Existing directory tests cover case-insensitive company search plus country/provider filters | Combine Canada/United States and provider filters in Admin settings | Complete |
| User-managed profiles | Regression checks remove Profiles from Admin settings and isolate statistics/results to Job matches | Add/edit/delete a profile from the user Profiles page | Complete |
| Global schedules | Typed API, `site_settings` migration, and read-only user card regression | Apply each schedule, inspect all profiles, and confirm the user card cannot be edited | Complete; Docker/PostgreSQL integration test pending |
| Site-wide sources | Admin authorization, migration synchronization, new-user inheritance, and navigation regression | Add/toggle/remove a source as admin, then compare two users | Complete; Docker/PostgreSQL integration test pending |
| User roles | Typed role-update API test and server authorization guard | Promote another user, verify access, demote them, and verify access is removed | Complete; Docker/PostgreSQL integration test pending |
| Data and diagnostics | Admin Data tab compiles and legacy navigation is unreachable | Export/import, run retention, and review diagnostics as admin | Complete |
| Admin page isolation | Source and CSS checks hide personal profile banner, metrics, Best job matches, and activity cards | Open each Admin settings tab at desktop and narrow widths | Complete |

## Release gate

Do not deploy to production until all of these pass on a machine with Docker and Rust:

```text
npm ci
npm run test:frontend
npm run build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
docker compose up -d --build
GET /api/health -> 200 {"status":"ok"}
desktop migration comparison -> no missing tracker data
cross-account authorization matrix -> no data leakage
Web Push with browser closed -> notification received
PostgreSQL backup restore drill -> successful
```
# Microsoft detail hydration (v0.19.0)

| Test | Expected result |
| --- | --- |
| Upgrade an existing database through migration `0013` | `sources.hydration_limit` and `connector_detail_cache` are created without changing prior migration checksums or deleting jobs. |
| Refresh Microsoft with an empty cache | At most the effective hydration limit is requested; normalized jobs include full descriptions for successfully hydrated records. |
| Refresh the same unchanged Microsoft results again | Matching summary fingerprints reuse cached details and make no detail request for those jobs. |
| Change a fingerprinted summary field | That job's detail is requested again and its cache record is replaced. |
| Exceed the hydration limit | Remaining new/changed jobs use summary or previously cached detail and remain eligible for hydration on a later refresh. |
| Return an invalid search or detail envelope | The source reports `MICROSOFT_SEARCH_CONTRACT_CHANGED`, `MICROSOFT_RESPONSE_CONTRACT_CHANGED`, or `MICROSOFT_DETAIL_CONTRACT_CHANGED`. |
| Return HTTP 429 or a transient 5xx response | The configured retry count, exponential backoff, and `Retry-After` handling apply. |
| Leave a cached detail unused for over 90 days | The worker removes it during routine maintenance. |
