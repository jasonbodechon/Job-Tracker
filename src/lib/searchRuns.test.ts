import { describe, expect, it, vi } from "vitest";
import type { SearchRun } from "../types/job";
import { MAX_CONSECUTIVE_POLL_FAILURES, runningSearch, searchRunResult, waitForSearchRun } from "./searchRuns";

const run = (overrides: Partial<SearchRun> = {}): SearchRun => ({ id: "r1", startedAt: "2026-10-23T14:00:00Z", triggerType: "manual", sourcesChecked: 2, jobsImported: 0, errors: [], status: "Running", progress: "Starting", ...overrides });
const noSleep = () => Promise.resolve();

describe("background search runs", () => {
  it("polls until the search finishes and reports progress along the way", async () => {
    const load = vi.fn()
      .mockResolvedValueOnce(run({ progress: "Amazon (source 1 of 2): fetching jobs" }))
      .mockResolvedValueOnce(run({ progress: "Amazon (source 1 of 2): checking job 26 of 1000" }))
      .mockResolvedValueOnce(run({ status: "Completed", progress: "Finished", jobsImported: 4, result: { imported: 4, inactive: 1 } }));
    const progress: string[] = [];
    const finished = await waitForSearchRun(load, "r1", { sleep: noSleep, onProgress: (current) => progress.push(current.progress) });
    expect(finished.status).toBe("Completed");
    expect(progress).toEqual(["Amazon (source 1 of 2): fetching jobs", "Amazon (source 1 of 2): checking job 26 of 1000"]);
    expect(load).toHaveBeenCalledTimes(3);
    expect(load).toHaveBeenCalledWith("r1");
  });

  it("retries brief network failures but gives up after repeated ones", async () => {
    const flaky = vi.fn().mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce(run({ status: "Completed" }));
    await expect(waitForSearchRun(flaky, "r1", { sleep: noSleep })).resolves.toMatchObject({ status: "Completed" });
    const down = vi.fn().mockRejectedValue(new Error("offline"));
    await expect(waitForSearchRun(down, "r1", { sleep: noSleep })).rejects.toThrow("offline");
    expect(down).toHaveBeenCalledTimes(MAX_CONSECUTIVE_POLL_FAILURES);
  });

  it("returns the stored result of a completed search", () => {
    expect(searchRunResult(run({ status: "Completed", result: { imported: 3, inactive: 2, fetched: 40 } }))).toEqual({ imported: 3, inactive: 2, fetched: 40 });
    expect(searchRunResult(run({ status: "Completed", jobsImported: 5, errors: ["Google: slow"] }))).toEqual({ imported: 5, inactive: 0, sourceErrors: ["Google: slow"] });
  });

  it("explains failed and abandoned searches", () => {
    expect(() => searchRunResult(run({ status: "Failed", errors: ["database operation failed"] }))).toThrow("database operation failed");
    expect(() => searchRunResult(run({ status: "Failed" }))).toThrow("The search failed.");
    expect(() => searchRunResult(run({ status: "Abandoned" }))).toThrow("The search stopped before finishing. Run it again.");
  });

  it("finds a search that is still running after the page reloads", () => {
    expect(runningSearch([run({ id: "r2" }), run({ id: "r1", status: "Completed" })])?.id).toBe("r2");
    expect(runningSearch([run({ status: "Completed" })])).toBeUndefined();
    expect(runningSearch(undefined)).toBeUndefined();
  });
});
