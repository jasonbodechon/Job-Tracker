import type { ImportResult, SearchRun } from "../types/job";

/** How often the page checks on a background search. */
export const SEARCH_POLL_INTERVAL_MS = 2000;
/** Brief network problems are retried; this many failed checks in a row stop following the search. */
export const MAX_CONSECUTIVE_POLL_FAILURES = 5;

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export interface FollowSearchOptions {
  onProgress?: (run: SearchRun) => void;
  intervalMs?: number;
  sleep?: (ms: number) => Promise<void>;
}

/** Polls a background search until it is no longer running and returns the finished run. */
export async function waitForSearchRun(load: (runId: string) => Promise<SearchRun>, runId: string, options: FollowSearchOptions = {}): Promise<SearchRun> {
  const { onProgress, intervalMs = SEARCH_POLL_INTERVAL_MS, sleep = wait } = options;
  let failures = 0;
  for (;;) {
    try {
      const run = await load(runId);
      failures = 0;
      if (run.status !== "Running") return run;
      onProgress?.(run);
    } catch (error) {
      failures += 1;
      if (failures >= MAX_CONSECUTIVE_POLL_FAILURES) throw error;
    }
    await sleep(intervalMs);
  }
}

/** Returns a completed run's import result, or throws an error that explains why it did not complete. */
export function searchRunResult(run: SearchRun): ImportResult {
  if (run.status === "Completed") return run.result ?? { imported: run.jobsImported, inactive: 0, sourceErrors: run.errors };
  const reason = run.errors.length ? run.errors[run.errors.length - 1] : undefined;
  if (run.status === "Abandoned") throw new Error(reason ?? "The search stopped before finishing. Run it again.");
  throw new Error(reason ?? "The search failed.");
}

/** The most recent search that is still running, if any, so a reloaded page can keep following it. */
export function runningSearch(runs: SearchRun[] | undefined): SearchRun | undefined {
  return runs?.find((run) => run.status === "Running");
}
