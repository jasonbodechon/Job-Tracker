import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const service = read("server/src/service.rs");
const api = read("server/src/api.rs");
const lib = read("server/src/lib.rs");
const geocoding = read("server/src/geocoding.rs");
const migration = read("server/migrations/0014_background_search_runs.sql");
const app = read("src/App.tsx");

describe("background job searches", () => {
  it("runs page-started searches in a background task that survives a closed request", () => {
    expect(service).toContain("pub async fn start_search(");
    expect(service).toContain("tokio::spawn(async move{execute_and_record(");
    expect(api).toContain("service::start_search(&state,u.id,&trigger,None)");
    expect(api).toContain("service::start_search(&state,u.id,\"manual\",Some(&id))");
    expect(api).toContain("StatusCode::ACCEPTED");
    expect(api).toContain('.route("/api/search-runs/{id}",get(search_run))');
  });

  it("allows only one running search per user and cleans up abandoned ones", () => {
    expect(migration).toContain("CREATE UNIQUE INDEX search_runs_one_running_per_user ON search_runs (user_id) WHERE status = 'Running'");
    expect(migration).toMatch(/SET status = 'Abandoned'[\s\S]*WHERE finished_at IS NULL/);
    expect(service).toContain("ON CONFLICT DO NOTHING");
    expect(service).toContain("ApiError::Conflict(ALREADY_RUNNING.into())");
    expect(service).toContain("pub async fn abandon_stale_runs(");
    expect(read("server/src/bin/api.rs")).toContain("abandon_interrupted_runs(&state,false)");
    expect(read("server/src/bin/worker.rs")).toContain("abandon_interrupted_runs(&state,true)");
  });

  it("records progress while a search runs", () => {
    expect(migration).toContain("ADD COLUMN progress text");
    expect(service).toContain("report_progress(state,user,run_id,&format!(\"{source_label}: fetching jobs\"))");
    expect(service).toContain("if posting_index%PROGRESS_EVERY==0");
  });

  it("limits outbound requests and does not cache failed location lookups", () => {
    expect(lib).toContain(".connect_timeout(HTTP_CONNECT_TIMEOUT).timeout(HTTP_REQUEST_TIMEOUT)");
    expect(geocoding).toContain(".timeout(Duration::from_secs(GEOCODING_TIMEOUT_SECONDS))");
    expect(geocoding).toContain("geocoding lookup failed; not caching");
    expect(geocoding).not.toContain("_ => Vec::new(),");
    expect(migration).toContain("DELETE FROM geocoding_cache WHERE results = '[]'::jsonb");
  });

  it("follows the search from the page and resumes it after a reload", () => {
    expect(app).toContain("waitForSearchRun(loadSearchRun, runId");
    expect(app).toContain("const running = runningSearch(runs)");
    expect(app).toContain('className="search-progress" role="status"');
  });
});
